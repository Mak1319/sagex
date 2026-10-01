//! Real voice notes (offline): mic capture → WAV vault file → preview
//! player → send. WhatsApp-style gestures: hold the mic to record (release
//! previews), tap to lock hands-free, tap stop to preview.
//!
//! Engine: `cpal` capture + `hound` WAV encode/decode; cpal output stream
//! for preview playback (no extra player dependency). All DSP state is
//! plain data; streams are stopped by dropping their handles.

use std::path::PathBuf;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

pub const SAMPLE_RATE: u32 = 16_000;
const BARS: usize = 48;

/// Live recorder. Dropping `stream` stops capture.
pub struct Recorder {
    samples: Arc<Mutex<Vec<f32>>>,
    rate: u32,
    started: Instant,
    _stream: cpal::Stream,
}

impl Recorder {
    pub fn start() -> Result<Self, String> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| "No microphone found.".to_string())?;
        let supported = device
            .default_input_config()
            .map_err(|e| format!("Mic unavailable: {e}"))?;
        // Normalize to mono f32 at the device rate; resample notes below.
        let rate = supported.sample_rate().0;
        let channels = supported.channels() as usize;
        let samples: Arc<Mutex<Vec<f32>>> =
            Arc::new(Mutex::new(Vec::with_capacity(SAMPLE_RATE as usize * 120)));
        let writer = samples.clone();
        let err_fn = |e| eprintln!("mic stream error: {e}");
        let stream = match supported.sample_format() {
            cpal::SampleFormat::F32 => device
                .build_input_stream(
                    &supported.config(),
                    move |data: &[f32], _| {
                        push_mono(&writer, data, channels);
                    },
                    err_fn,
                    None,
                )
                .map_err(|e| format!("Mic unavailable: {e}"))?,
            cpal::SampleFormat::I16 => device
                .build_input_stream(
                    &supported.config(),
                    move |data: &[i16], _| {
                        let conv: Vec<f32> =
                            data.iter().map(|s| *s as f32 / i16::MAX as f32).collect();
                        push_mono(&writer, &conv, channels);
                    },
                    err_fn,
                    None,
                )
                .map_err(|e| format!("Mic unavailable: {e}"))?,
            cpal::SampleFormat::U16 => device
                .build_input_stream(
                    &supported.config(),
                    move |data: &[u16], _| {
                        let conv: Vec<f32> = data
                            .iter()
                            .map(|s| (*s as f32 / u16::MAX as f32) * 2.0 - 1.0)
                            .collect();
                        push_mono(&writer, &conv, channels);
                    },
                    err_fn,
                    None,
                )
                .map_err(|e| format!("Mic unavailable: {e}"))?,
            _ => return Err("Unsupported mic format.".to_string()),
        };
        stream.play().map_err(|e| format!("Mic unavailable: {e}"))?;
        Ok(Self {
            samples,
            rate,
            started: Instant::now(),
            _stream: stream,
        })
    }

    pub fn elapsed(&self) -> Duration {
        self.started.elapsed()
    }

    /// Snapshot samples so far (for live bars).
    pub fn snapshot(&self) -> Vec<f32> {
        self.samples.lock().unwrap().clone()
    }

    /// Stop capture, write 16kHz mono WAV to the vault, return the file.
    pub fn finish(self, room_key: &str) -> Result<(PathBuf, u64, Vec<u32>), String> {
        let raw = self.snapshot();
        drop(self._stream);
        let mono = to_mono_16k(&raw, self.rate);
        let secs = (mono.len() as f32 / SAMPLE_RATE as f32).max(0.0);
        if secs < 0.5 {
            return Err("Too short — hold a little longer.".to_string());
        }
        let dir = crate::chat::files::vault_dir(room_key);
        std::fs::create_dir_all(&dir).map_err(|e| format!("Vault error: {e}"))?;
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let path = dir.join(format!("voicenote-{stamp}.wav"));
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: SAMPLE_RATE,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut w =
            hound::WavWriter::create(&path, spec).map_err(|e| format!("Vault error: {e}"))?;
        for s in &mono {
            w.write_sample((s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)
                .map_err(|e| format!("Vault error: {e}"))?;
        }
        w.finalize().map_err(|e| format!("Vault error: {e}"))?;
        Ok((path, secs.round() as u64, wave_bars(&mono)))
    }
}

fn push_mono(dst: &Arc<Mutex<Vec<f32>>>, interleaved: &[f32], channels: usize) {
    if channels == 0 {
        return;
    }
    let mut lock = dst.lock().unwrap();
    // Cap at ~10 minutes to bound memory.
    if lock.len() > SAMPLE_RATE as usize * 600 {
        return;
    }
    for frame in interleaved.chunks(channels) {
        lock.push(frame.iter().sum::<f32>() / channels as f32);
    }
}

/// Downmix-resample to 16kHz mono (nearest-neighbor; voice notes only).
fn to_mono_16k(raw: &[f32], rate: u32) -> Vec<f32> {
    if rate == SAMPLE_RATE || rate == 0 {
        return raw.to_vec();
    }
    let ratio = rate as f32 / SAMPLE_RATE as f32;
    let out_len = (raw.len() as f32 / ratio) as usize;
    (0..out_len)
        .map(|i| raw[((i as f32 * ratio) as usize).min(raw.len().saturating_sub(1))])
        .collect()
}

/// 28 RMS bars for waveforms (0..22 like the bubble renderer).
pub fn wave_bars(samples: &[f32]) -> Vec<u32> {
    if samples.is_empty() {
        return vec![4; BARS];
    }
    let chunk = (samples.len() / BARS).max(1);
    (0..BARS)
        .map(|b| {
            let s = b * chunk;
            let e = ((b + 1) * chunk).min(samples.len());
            let rms = if e > s {
                (samples[s..e].iter().map(|x| x * x).sum::<f32>() / (e - s) as f32).sqrt()
            } else {
                0.0
            };
            (4.0 + rms * 60.0).clamp(4.0, 22.0) as u32
        })
        .collect()
}

/// Decode a vault WAV back to mono f32 (any rate; resampled to 16k).
pub fn decode_wav(path: &std::path::Path) -> Result<Vec<f32>, String> {
    let mut r = hound::WavReader::open(path).map_err(|_| "Can't read audio.".to_string())?;
    let spec = r.spec();
    let chans = spec.channels.max(1) as usize;
    let raw: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Int => r
            .samples::<i32>()
            .map(|s| s.unwrap_or(0) as f32 / (1 << (spec.bits_per_sample - 1)) as f32)
            .collect(),
        hound::SampleFormat::Float => r.samples::<f32>().map(|s| s.unwrap_or(0.0)).collect(),
    };
    let mono: Vec<f32> = raw
        .chunks(chans)
        .map(|f| f.iter().sum::<f32>() / chans as f32)
        .collect();
    Ok(to_mono_16k(&mono, spec.sample_rate))
}

/// Preview playback handle. Dropping `stream` stops audio.
pub struct PlayHandle {
    _stream: cpal::Stream,
}

pub fn play_samples(samples: Arc<Vec<f32>>) -> Result<PlayHandle, String> {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or_else(|| "No speaker found.".to_string())?;
    let supported = device
        .default_output_config()
        .map_err(|e| format!("Speaker unavailable: {e}"))?;
    // Resample 16k → device rate (nearest-neighbor).
    let out_rate = supported.sample_rate().0;
    let ratio = out_rate as f32 / SAMPLE_RATE as f32;
    let up: Vec<f32> = (0..(samples.len() as f32 * ratio) as usize)
        .map(|i| samples[((i as f32 / ratio) as usize).min(samples.len().saturating_sub(1))])
        .collect();
    let channels = supported.channels() as usize;
    let total_frames = up.len();
    let shared = Arc::new(PlayState {
        up,
        cursor: AtomicUsize::new(0),
        channels,
        total_frames,
    });
    let err_fn = |e| eprintln!("speaker error: {e}");
    let stream = match supported.sample_format() {
        cpal::SampleFormat::F32 => {
            let st = shared.clone();
            device
                .build_output_stream(
                    &supported.config(),
                    move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                        render_into(&st, data);
                    },
                    err_fn,
                    None,
                )
                .map_err(|e| format!("Speaker unavailable: {e}"))?
        }
        cpal::SampleFormat::I16 => {
            let st = shared.clone();
            device
                .build_output_stream(
                    &supported.config(),
                    move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                        let mut tmp = vec![0.0f32; data.len()];
                        render_into(&st, &mut tmp);
                        for (o, s) in data.iter_mut().zip(tmp) {
                            *o = (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
                        }
                    },
                    err_fn,
                    None,
                )
                .map_err(|e| format!("Speaker unavailable: {e}"))?
        }
        cpal::SampleFormat::U16 => {
            let st = shared.clone();
            device
                .build_output_stream(
                    &supported.config(),
                    move |data: &mut [u16], _: &cpal::OutputCallbackInfo| {
                        let mut tmp = vec![0.0f32; data.len()];
                        render_into(&st, &mut tmp);
                        for (o, s) in data.iter_mut().zip(tmp) {
                            *o = ((s.clamp(-1.0, 1.0) + 1.0) / 2.0 * u16::MAX as f32) as u16;
                        }
                    },
                    err_fn,
                    None,
                )
                .map_err(|e| format!("Speaker unavailable: {e}"))?
        }
        _ => return Err("Unsupported speaker format.".to_string()),
    };
    stream
        .play()
        .map_err(|e| format!("Speaker unavailable: {e}"))?;
    Ok(PlayHandle { _stream: stream })
}

struct PlayState {
    up: Vec<f32>,
    cursor: AtomicUsize,
    channels: usize,
    total_frames: usize,
}

fn render_into(st: &PlayState, data: &mut [f32]) {
    for frame in data.chunks_mut(st.channels) {
        let i = st.cursor.fetch_add(1, Ordering::Relaxed);
        let s = if i < st.total_frames { st.up[i] } else { 0.0 };
        for x in frame.iter_mut() {
            *x = s;
        }
    }
}

pub fn fmt_duration(total_secs: u64) -> String {
    format!("{}:{:02}", total_secs / 60, total_secs % 60)
}

// ---------------------------------------------------------------------------
// UI state machine + HUD (WhatsApp mimic).
// ---------------------------------------------------------------------------

use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, Timer, div, px,
};
use gpui_component::{ActiveTheme, Icon, Sizable};

use super::{
    ChatApp, files::Attachment, files::FileKind, model::MessageKind, model::MessageStatus,
};

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum AudioUi {
    #[default]
    Idle,
    Recording {
        locked: bool,
    },
    Preview,
}

#[derive(Clone, Debug)]
pub struct PreviewClip {
    pub path: std::path::PathBuf,
    pub secs: u64,
    pub bars: Vec<u32>,
    pub size: u64,
}

impl ChatApp {
    fn audio_room_key(&self) -> String {
        self.room_server
            .get(&self.active_id)
            .cloned()
            .unwrap_or_else(|| format!("local-{}", self.active_id))
    }

    /// Tap path (mic button / Audio row): toggle locked recording, or stop
    /// an active take into the preview.
    pub fn audio_toggle(&mut self, cx: &mut Context<Self>) {
        match self.audio_ui {
            AudioUi::Idle => self.audio_start(true, cx),
            AudioUi::Recording { .. } => self.audio_stop_to_preview(cx),
            AudioUi::Preview => {}
        }
    }

    /// Hold path: press starts an unlocked take.
    pub fn audio_hold_start(&mut self, cx: &mut Context<Self>) {
        if self.audio_ui != AudioUi::Idle {
            return;
        }
        self.mic_down_at = Some(Instant::now());
        self.audio_start(false, cx);
    }

    /// Hold path: release stops to preview — unless it was a quick tap,
    /// which converts to locked hands-free recording.
    pub fn audio_hold_stop(&mut self, cx: &mut Context<Self>) {
        let quick = self
            .mic_down_at
            .map(|t| t.elapsed() < Duration::from_millis(300))
            .unwrap_or(false);
        self.mic_down_at = None;
        match self.audio_ui {
            AudioUi::Recording { .. } if quick => {
                self.audio_ui = AudioUi::Recording { locked: true };
                cx.notify();
            }
            AudioUi::Recording { .. } => self.audio_stop_to_preview(cx),
            _ => {}
        }
    }

    fn audio_start(&mut self, locked: bool, cx: &mut Context<Self>) {
        match Recorder::start() {
            Ok(rec) => {
                self.recorder = Some(rec);
                self.playing = None;
                self.clip = None;
                self.audio_ui = AudioUi::Recording { locked };
                self.audio_gen += 1;
                let mark = self.audio_gen;
                // 4fps ticker for the timer + live bars.
                cx.spawn(async move |weak, cx| {
                    let ex = cx.background_executor().clone();
                    loop {
                        ex.spawn(async {
                            Timer::after(Duration::from_millis(250)).await;
                        })
                        .await;
                        let go: bool = weak
                            .update(cx, |v: &mut ChatApp, cx| {
                                let on = v.audio_gen == mark
                                    && matches!(v.audio_ui, AudioUi::Recording { .. });
                                if on {
                                    cx.notify();
                                }
                                on
                            })
                            .unwrap_or(false);
                        if !go {
                            break;
                        }
                    }
                })
                .detach();
                cx.notify();
            }
            Err(e) => {
                self.notice = Some(e);
                cx.notify();
            }
        }
    }

    fn audio_stop_to_preview(&mut self, cx: &mut Context<Self>) {
        let room = self.audio_room_key();
        let rec = self.recorder.take();
        self.audio_gen += 1; // halt the ticker
        match rec {
            Some(r) => match r.finish(&room) {
                Ok((path, secs, bars)) => {
                    let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                    self.clip = Some(PreviewClip {
                        path,
                        secs,
                        bars,
                        size,
                    });
                    self.audio_ui = AudioUi::Preview;
                    cx.notify();
                }
                Err(e) => {
                    self.audio_ui = AudioUi::Idle;
                    self.notice = Some(e);
                    cx.notify();
                }
            },
            None => {
                self.audio_ui = AudioUi::Idle;
                cx.notify();
            }
        }
    }

    pub fn audio_discard(&mut self, cx: &mut Context<Self>) {
        self.audio_gen += 1;
        self.recorder = None;
        self.playing = None;
        if let Some(c) = self.clip.take() {
            let _ = std::fs::remove_file(c.path);
        }
        self.audio_ui = AudioUi::Idle;
        cx.notify();
    }

    pub fn audio_play(&mut self, cx: &mut Context<Self>) {
        if self.playing.is_some() {
            self.playing = None; // pause = stop; replay restarts
            cx.notify();
            return;
        }
        let Some(clip) = self.clip.clone() else {
            return;
        };
        let samples = match decode_wav(&clip.path) {
            Ok(s) => std::sync::Arc::new(s),
            Err(e) => {
                self.notice = Some(e);
                cx.notify();
                return;
            }
        };
        match play_samples(samples) {
            Ok(h) => {
                self.playing = Some(h);
                self.audio_gen += 1;
                let mark = self.audio_gen;
                let secs = clip.secs.max(1);
                cx.spawn(async move |weak, cx| {
                    let ex = cx.background_executor().clone();
                    ex.spawn(async move {
                        Timer::after(Duration::from_secs(secs)).await;
                    })
                    .await;
                    let _ = weak.update(cx, |v: &mut ChatApp, cx| {
                        if v.audio_gen == mark {
                            v.playing = None;
                            cx.notify();
                        }
                    });
                })
                .detach();
                cx.notify();
            }
            Err(e) => {
                self.notice = Some(e);
                cx.notify();
            }
        }
    }

    /// Send the previewed take as a real voice attachment message.
    pub fn audio_send(&mut self, cx: &mut Context<Self>) {
        let Some(clip) = self.clip.take() else {
            return;
        };
        self.playing = None;
        self.audio_ui = AudioUi::Idle;
        let name = clip
            .path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("voice note.wav")
            .to_string();
        let label = format!("Voice note ({})", fmt_duration(clip.secs));
        let id = self.next_msg;
        self.next_msg += 1;
        let my_name = self.my_name();
        if let Some(c) = self.chats.iter_mut().find(|c| c.id == self.active_id) {
            c.messages.push(super::model::Message {
                id,
                sender: my_name,
                link: None,
                text: label,
                time: "now".to_string(),
                mine: true,
                date: "Today".to_string(),
                kind: MessageKind::Voice {
                    duration: fmt_duration(clip.secs),
                    bars: clip.bars.clone(),
                },
                reactions: vec![],
                reply_to: None,
                ticks: MessageStatus::Sent,
                deleted: false,
                seal: None,
                server_id: None,
                attachment: Some(Attachment {
                    path: clip.path,
                    name,
                    size: clip.size,
                    kind: FileKind::Audio,
                    locked: false,
                }),
            });
            c.last_time = "now".to_string();
        }
        cx.notify();
    }

    fn audio_elapsed(&self) -> Duration {
        self.recorder
            .as_ref()
            .map(|r| r.elapsed())
            .unwrap_or_default()
    }

    /// Recording / preview HUD (reference-mock layout, theme styling):
    /// single bar — status dot, dense centered waveform, timer, trash,
    /// transport pill, send. `None` when idle (composer shows instead).
    pub(super) fn render_audio_hud(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let theme = cx.theme().clone();
        match &self.audio_ui {
            AudioUi::Idle => None,
            AudioUi::Recording { locked } => {
                let locked = *locked;
                let secs = self.audio_elapsed().as_secs();
                let bars = self
                    .recorder
                    .as_ref()
                    .map(|r| wave_bars(&r.snapshot()))
                    .unwrap_or_else(|| vec![4; BARS]);
                Some(
                    Self::hud_bar(cx, &theme)
                        // red dot in a ring
                        .child(
                            div()
                                .size(px(40.))
                                .flex_shrink_0()
                                .rounded_full()
                                .border_2()
                                .border_color(theme.danger)
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(div().size(px(14.)).rounded_full().bg(theme.danger)),
                        )
                        .child(Self::hud_wave(&theme, &bars, true))
                        .child(
                            div()
                                .text_sm()
                                .flex_shrink_0()
                                .text_color(theme.muted_foreground)
                                .child(fmt_duration(secs)),
                        )
                        .child(Self::hud_divider(&theme))
                        .child(Self::hud_box(
                            cx,
                            "rec-del",
                            "icons/trash-2.svg",
                            theme.danger,
                            true,
                            |this, cx| this.audio_discard(cx),
                        ))
                        .child(Self::hud_divider(&theme))
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap_2()
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme.muted_foreground)
                                        .child(if locked { "Locked" } else { "" }),
                                )
                                .child(Self::hud_box(
                                    cx,
                                    "rec-stop",
                                    "icons/square.svg",
                                    theme.primary_foreground,
                                    false,
                                    |this, cx| this.audio_stop_to_preview(cx),
                                )),
                        )
                        .into_any_element(),
                )
            }
            AudioUi::Preview => {
                let clip = self.clip.clone()?;
                let playing = self.playing.is_some();
                Some(
                    Self::hud_bar(cx, &theme)
                        .child(Self::hud_transport(cx, &theme, playing))
                        .child(Self::hud_wave(&theme, &clip.bars, playing))
                        .child(
                            div()
                                .text_sm()
                                .flex_shrink_0()
                                .text_color(theme.muted_foreground)
                                .child(fmt_duration(clip.secs)),
                        )
                        .child(Self::hud_divider(&theme))
                        .child(Self::hud_box(
                            cx,
                            "clip-del",
                            "icons/trash-2.svg",
                            theme.danger,
                            true,
                            |this, cx| this.audio_discard(cx),
                        ))
                        .child(Self::hud_divider(&theme))
                        .child(Self::hud_box(
                            cx,
                            "clip-send",
                            "icons/send.svg",
                            theme.primary_foreground,
                            false,
                            |this, cx| this.audio_send(cx),
                        ))
                        .into_any_element(),
                )
            }
        }
    }

    /// Shared bar shell for the recorder HUD.
    fn hud_bar(cx: &mut Context<Self>, theme: &gpui_component::Theme) -> gpui::Div {
        let _ = cx;
        div()
            .w_full()
            .h(px(64.))
            .flex()
            .flex_row()
            .items_center()
            .gap_3()
            .rounded(px(20.))
            .border_1()
            .border_color(theme.border)
            .bg(theme.popover)
            .shadow_lg()
            .px_4()
    }

    /// Dense centered waveform: played bars tinted, rest muted.
    fn hud_wave(theme: &gpui_component::Theme, bars: &[u32], hot: bool) -> AnyElement {
        let mut row = div().flex_1().flex().flex_row().items_center().gap(px(2.));
        for (ix, h) in bars.iter().enumerate() {
            row = row.child(div().w(px(3.)).h(px(*h as f32)).rounded_full().bg(
                if hot && ix % 2 == 0 {
                    theme.danger
                } else {
                    theme.muted_foreground
                },
            ));
        }
        row.into_any_element()
    }

    fn hud_divider(theme: &gpui_component::Theme) -> AnyElement {
        div()
            .w(px(1.))
            .h(px(32.))
            .flex_shrink_0()
            .bg(theme.border)
            .into_any_element()
    }

    /// 48px rounded action box. `outline` renders danger-outline (trash),
    /// otherwise solid primary.
    fn hud_box(
        cx: &mut Context<Self>,
        id: &'static str,
        icon: &'static str,
        fg: gpui::Hsla,
        outline: bool,
        on_tap: impl Fn(&mut Self, &mut Context<Self>) + 'static,
    ) -> AnyElement {
        let theme = cx.theme().clone();
        let mut btn = div()
            .id(id)
            .size(px(48.))
            .flex_shrink_0()
            .rounded(px(14.))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .text_color(fg);
        btn = if outline {
            btn.border_1()
                .border_color(theme.danger)
                .bg(theme.background)
        } else {
            btn.bg(theme.primary)
        };
        btn.child(Icon::empty().path(icon).large())
            .on_click(cx.listener(move |this, _, _, cx| {
                on_tap(this, cx);
            }))
            .into_any_element()
    }

    /// Play | pause transport pill (reference layout): left half plays,
    /// right half pauses; the active side reads bright.
    fn hud_transport(
        cx: &mut Context<Self>,
        theme: &gpui_component::Theme,
        playing: bool,
    ) -> AnyElement {
        let half = |active: bool| {
            if active {
                theme.foreground
            } else {
                theme.muted_foreground
            }
        };
        div()
            .flex_shrink_0()
            .flex()
            .flex_row()
            .items_center()
            .h(px(48.))
            .rounded_full()
            .border_1()
            .border_color(theme.border)
            .bg(theme.background)
            .child(
                div()
                    .id("clip-seek-play")
                    .w(px(52.))
                    .h_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .text_color(half(!playing))
                    .child(Icon::empty().path("icons/play.svg").large())
                    .on_click(cx.listener(|this, _, _, cx| {
                        if this.playing.is_none() {
                            this.audio_play(cx);
                        }
                    })),
            )
            .child(div().w(px(1.)).h(px(24.)).bg(theme.border))
            .child(
                div()
                    .id("clip-seek-pause")
                    .w(px(52.))
                    .h_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .text_color(half(playing))
                    .child(Icon::empty().path("icons/pause.svg").large())
                    .on_click(cx.listener(|this, _, _, cx| {
                        if this.playing.is_some() {
                            this.audio_play(cx);
                        }
                    })),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bars_shape() {
        let bars = wave_bars(&[]);
        assert_eq!(bars.len(), 48);
        let loud = wave_bars(&vec![0.9; 16000]);
        assert!(loud.iter().all(|b| *b >= 4 && *b <= 22));
        assert!(loud.iter().sum::<u32>() > bars.iter().sum::<u32>());
    }

    #[test]
    fn duration_format() {
        assert_eq!(fmt_duration(0), "0:00");
        assert_eq!(fmt_duration(7), "0:07");
        assert_eq!(fmt_duration(65), "1:05");
    }

    #[test]
    fn wav_roundtrip() {
        let dir = std::env::temp_dir().join("sagex_wav_test");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("t.wav");
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: SAMPLE_RATE,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut w = hound::WavWriter::create(&path, spec).unwrap();
        for i in 0..SAMPLE_RATE {
            w.write_sample(((i as f32 / SAMPLE_RATE as f32 * 440.0 * 2.0).sin() * 10000.0) as i16)
                .unwrap();
        }
        w.finalize().unwrap();
        let back = decode_wav(&path).unwrap();
        assert!((back.len() as i64 - SAMPLE_RATE as i64).abs() < 10);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
