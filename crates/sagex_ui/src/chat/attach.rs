//! Attachment flows (offline-first): stage from drop/picker → vault copy
//! with completion → preview tray → send as File bubbles.
//!
//! Locked mode is UI-only for now: the toggle + passphrase are collected in
//! the tray, messages carry the marker, and the real cipher plugs into the
//! `lock_bytes` seam in [`files`](super::files). Nothing here claims bytes
//! are encrypted.

use std::path::{Path, PathBuf};
use std::time::Duration;

use gpui::{Context, Timer};

use super::{
    ChatApp,
    files::{self, Attachment, FileKind},
    model::{Message, MessageKind, MessageStatus, extract_link, voice_bars},
};

/// Full-size preview overlay selection (sent or staged image).
#[derive(Clone, Debug)]
pub struct PreviewSel {
    pub path: PathBuf,
    pub name: String,
}

impl ChatApp {
    fn room_key(&self) -> String {
        self.room_server
            .get(&self.active_id)
            .cloned()
            .unwrap_or_else(|| format!("local-{}", self.active_id))
    }

    /// Stage dropped/picked paths: validate synchronously, copy to the vault
    /// on background threads, refresh the tray as each finishes.
    pub fn stage_paths(&mut self, paths: &[PathBuf], cx: &mut Context<Self>) {
        let room = self.room_key();
        let budget = files::MAX_DROP_FILES.saturating_sub(self.staged.len());
        if budget == 0 {
            self.notice = Some(format!("At most {} files at once.", files::MAX_DROP_FILES));
            cx.notify();
            return;
        }
        if paths.len() > budget {
            self.notice = Some(format!(
                "Taking the first {budget} of {} files.",
                paths.len()
            ));
        }
        let mut skipped = vec![];
        for src in paths.iter().take(budget) {
            let src = src.clone();
            match files::inspect(&src) {
                Ok((name, size, kind)) => {
                    let id = self.stage_seq;
                    self.stage_seq += 1;
                    self.staged.push(files::StagedFile {
                        id,
                        name: name.clone(),
                        size,
                        kind,
                        vault_path: None,
                        progress: 0.0,
                        error: None,
                    });
                    // blocking vault copy off the foreground thread
                    let dst = files::vault_dest(&room, &name);
                    crate::backend::request(
                        cx,
                        async move {
                            files::copy_with_progress(&src, &dst, size, |_| {})
                                .map_err(|e| format!("Copy failed: {e}"))?;
                            Ok::<_, String>(dst)
                        },
                        move |this, res: Result<PathBuf, String>, cx| {
                            if let Some(s) = this.staged.iter_mut().find(|s| s.id == id) {
                                match res {
                                    Ok(dst) => {
                                        s.vault_path = Some(dst);
                                        s.progress = 1.0;
                                    }
                                    Err(e) => s.error = Some(e),
                                }
                            }
                            cx.notify();
                        },
                    )
                    .detach();
                }
                Err(e) => skipped.push(format!(
                    "{} — {e}",
                    src.file_name().and_then(|n| n.to_str()).unwrap_or("file")
                )),
            }
        }
        if !skipped.is_empty() {
            self.notice = Some(format!("Skipped: {}", skipped.join("; ")));
        }
        cx.notify();
    }

    pub fn unstage(&mut self, id: usize, cx: &mut Context<Self>) {
        self.staged.retain(|s| s.id != id);
        cx.notify();
    }

    pub fn clear_staged(&mut self, cx: &mut Context<Self>) {
        self.staged.clear();
        self.stage_locked = false;
        cx.notify();
    }

    /// Send every fully-copied staged file as a File-bubble message.
    pub fn send_staged(&mut self, cx: &mut Context<Self>) {
        let caption = self.stage_caption.read(cx).value().trim().to_string();
        // Locked mode is UI-only for now (no passphrase gate — the real
        // scheme lands later); files just carry the marker.
        let locked = self.stage_locked;
        let ready: Vec<_> = self.staged.iter().filter(|s| s.done()).cloned().collect();
        if ready.is_empty() {
            let waiting = self.staged.iter().any(|s| s.error.is_none());
            self.notice = Some(if waiting {
                "Files are still copying — send in a moment.".to_string()
            } else {
                "Nothing to send — drop or pick files first.".to_string()
            });
            cx.notify();
            return;
        }
        let copying: Vec<_> = self
            .staged
            .iter()
            .filter(|s| !s.done() && s.error.is_none())
            .cloned()
            .collect();
        for sf in ready {
            let vault = sf.vault_path.clone().unwrap();
            let bytes = std::fs::read(&vault).unwrap_or_default();
            // Locked-mode seam (UI-only): bytes pass through unchanged until
            // the real scheme lands. Marker below is honest UI state.
            let _ = files::lock_bytes(bytes, "");
            let att = Attachment {
                path: vault,
                name: sf.name.clone(),
                size: sf.size,
                kind: sf.kind,
                locked,
            };
            let text = if caption.is_empty() {
                sf.name.clone()
            } else {
                caption.clone()
            };
            let kind = match sf.kind {
                FileKind::Image => MessageKind::Image {
                    caption: caption.clone(),
                },
                FileKind::Audio => MessageKind::Voice {
                    duration: files::fmt_size(sf.size),
                    bars: voice_bars(self.next_msg),
                },
                _ => MessageKind::Document {
                    name: sf.name.clone(),
                    size: files::fmt_size(sf.size),
                },
            };
            let id = self.next_msg;
            self.next_msg += 1;
            let my_name = self.my_name();
            if let Some(c) = self.chats.iter_mut().find(|c| c.id == self.active_id) {
                c.messages.push(Message {
                    id,
                    sender: my_name,
                    link: extract_link(&text),
                    text,
                    time: "now".to_string(),
                    mine: true,
                    date: "Today".to_string(),
                    kind,
                    reactions: vec![],
                    reply_to: None,
                    ticks: MessageStatus::Sent,
                    deleted: false,
                    seal: None,
                    server_id: None,
                    attachment: Some(att),
                });
                c.last_time = "now".to_string();
            }
        }
        // keep still-copying items staged; drop the sent ones
        self.staged = copying;
        self.stage_locked = false;
        cx.notify();
    }

    /// Native file picker → staged tray (multi-select).
    pub fn pick_files(&mut self, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some("Attach files".into()),
        });
        cx.spawn(async move |view, cx| {
            let out = cx.background_executor().spawn(rx).await;
            match out {
                Ok(Ok(Some(paths))) if !paths.is_empty() => {
                    let _ = view.update(cx, |v: &mut ChatApp, cx| {
                        v.stage_paths(&paths, cx);
                    });
                }
                _ => {}
            }
        })
        .detach();
    }

    /// Open a vault file with the OS default app (best effort).
    pub fn open_path(path: &Path) {
        #[cfg(target_os = "linux")]
        let _ = std::process::Command::new("xdg-open").arg(path).spawn();
        #[cfg(target_os = "macos")]
        let _ = std::process::Command::new("open").arg(path).spawn();
        #[cfg(target_os = "windows")]
        let _ = std::process::Command::new("cmd")
            .args(["/C", "start", "", &path.to_string_lossy()])
            .spawn();
    }

    // ---------- drag & drop ----------
    /// Called on every drag-move over the chat column; auto-clears if the
    /// drag leaves without dropping (platform sends no leave event).
    pub fn note_drag(&mut self, cx: &mut Context<Self>) {
        self.drag_hover = true;
        self.hover_gen += 1;
        let mark = self.hover_gen;
        let view = cx.entity();
        cx.spawn(async move |_, cx| {
            let ex = cx.background_executor().clone();
            ex.spawn(async move {
                Timer::after(Duration::from_millis(1500)).await;
            })
            .await;
            let _ = view.update(cx, |v: &mut ChatApp, cx| {
                if v.hover_gen == mark {
                    v.drag_hover = false;
                    cx.notify();
                }
            });
        })
        .detach();
        cx.notify();
    }

    pub fn drop_paths(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        self.drag_hover = false;
        self.hover_gen += 1;
        self.stage_paths(&paths, cx);
    }
}
