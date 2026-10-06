use std::{ops::Range, rc::Rc};

use gpui_kit::{
    component::input::{
        EditorState, FoldRange, HighlightStyleResolver, InputEdit, InputHighlighter,
        InputHighlighterFactory, Rope,
    },
    prelude::*,
    *,
};

#[derive(Clone, Copy, PartialEq)]
enum MdKind {
    Bold,
    Italic,
    Code,
    Marker,
}

struct MdRun {
    range: Range<usize>,
    kind: MdKind,
}

/// Heuristic chat-markdown parser (`**bold**`, `*italic*`, `` `code` ``,
/// ``` fences). Shared by the live rendered preview below.
fn parse(text: &str) -> Vec<MdRun> {
    let mut runs = Vec::new();
    let mut in_fence = false;
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        let len = line.len();
        let body = line.strip_suffix('\n').unwrap_or(line);
        if body.starts_with("```") {
            in_fence = !in_fence;
        } else if in_fence {
            runs.push(MdRun {
                range: offset..offset + len,
                kind: MdKind::Code,
            });
        } else {
            parse_inline(body, offset, &mut runs);
        }
        offset += len;
    }
    runs
}

fn push(runs: &mut Vec<MdRun>, range: Range<usize>, kind: MdKind) {
    if !range.is_empty() {
        runs.push(MdRun { range, kind });
    }
}

/// Concealing chat-markdown highlighter for the MESSAGE field: markers
/// render fully transparent (pure rendered look) while inner text shows
/// bold / italic / code styling. Display-only — the source keeps markers,
/// so caret, selection and toolbar offsets stay exact.
pub struct MdHighlighter {
    runs: Vec<MdRun>,
    code_bg: Hsla,
    code_fg: Hsla,
    marker_bg: Hsla,
}

impl MdHighlighter {
    pub fn new(code_bg: Hsla, code_fg: Hsla, marker_bg: Hsla) -> Self {
        Self {
            runs: Vec::new(),
            code_bg,
            code_fg,
            marker_bg,
        }
    }

    fn style_for(&self, kind: MdKind) -> HighlightStyle {
        match kind {
            MdKind::Bold => HighlightStyle {
                font_weight: Some(FontWeight::BOLD),
                ..Default::default()
            },
            MdKind::Italic => HighlightStyle {
                font_style: Some(FontStyle::Italic),
                ..Default::default()
            },
            MdKind::Code => HighlightStyle {
                color: Some(self.code_fg),
                background_color: Some(self.code_bg),
                ..Default::default()
            },
            // Concealed: fully faded AND painted in field-bg color, so
            // markers vanish whether or not the renderer honors fade.
            // (Layout width remains — accepted v1 gap.)
            MdKind::Marker => HighlightStyle {
                color: Some(self.marker_bg),
                background_color: Some(self.marker_bg),
                fade_out: Some(1.0),
                ..Default::default()
            },
        }
    }

    fn parse_runs(text: &str) -> Vec<MdRun> {
        parse(text)
    }
}

impl InputHighlighter for MdHighlighter {
    fn language(&self) -> SharedString {
        "chat-markdown".into()
    }

    fn update(
        &mut self,
        _edit: Option<InputEdit>,
        text: &Rope,
        _folding: bool,
        _window: &mut Window,
        _cx: &mut Context<EditorState>,
    ) {
        self.runs = Self::parse_runs(&text.to_string());
    }

    fn styles(
        &self,
        range: &Range<usize>,
        _resolver: &dyn HighlightStyleResolver,
    ) -> Vec<(Range<usize>, HighlightStyle)> {
        let mut out = Vec::new();
        let mut cursor = range.start;
        for run in &self.runs {
            if run.range.end <= range.start || run.range.start >= range.end {
                continue;
            }
            let s = run.range.start.max(range.start);
            let e = run.range.end.min(range.end);
            if cursor < s {
                out.push((cursor..s, HighlightStyle::default()));
            }
            out.push((s..e, self.style_for(run.kind)));
            cursor = e;
        }
        if cursor < range.end {
            out.push((cursor..range.end, HighlightStyle::default()));
        }
        out
    }

    fn fold_ranges(&self, _text: &Rope) -> Vec<FoldRange> {
        Vec::new()
    }
}

/// Factory for the MESSAGE field. Colors originate from the crate theme
/// at construction — never hardcoded here.
pub fn chat_markdown_factory(
    code_bg: Hsla,
    code_fg: Hsla,
    marker_bg: Hsla,
) -> InputHighlighterFactory {
    Rc::new(move |name| {
        if name == "chat-markdown" {
            let highlighter = MdHighlighter::new(code_bg, code_fg, marker_bg);
            Some(Box::new(highlighter) as Box<dyn InputHighlighter>)
        } else {
            None
        }
    })
}

fn parse_inline(line: &str, base: usize, runs: &mut Vec<MdRun>) {
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'*' {
            let bold = i + 1 < bytes.len() && bytes[i + 1] == b'*';
            let m = if bold { 2 } else { 1 };
            let rest = &line[i + m..];
            let closer = if bold { rest.find("**") } else { rest.find('*') };
            match closer {
                Some(rel) if rel > 0 => {
                    push(runs, base + i..base + i + m, MdKind::Marker);
                    push(
                        runs,
                        base + i + m..base + i + m + rel,
                        if bold { MdKind::Bold } else { MdKind::Italic },
                    );
                    push(
                        runs,
                        base + i + m + rel..base + i + m + rel + m,
                        MdKind::Marker,
                    );
                    i += m + rel + m;
                }
                _ => i += m,
            }
        } else if bytes[i] == b'`' {
            match line[i + 1..].find('`') {
                Some(rel) if rel > 0 => {
                    push(runs, base + i..base + i + 1, MdKind::Marker);
                    push(runs, base + i + 1..base + i + 1 + rel, MdKind::Code);
                    push(
                        runs,
                        base + i + 1 + rel..base + i + 1 + rel + 1,
                        MdKind::Marker,
                    );
                    i += 1 + rel + 1;
                }
                _ => i += 1,
            }
        } else {
            i += 1;
        }
    }
}

/// Clamp an offset down to a character boundary.
fn clip(text: &str, mut i: usize) -> usize {
    i = i.min(text.len());
    while i > 0 && !text.is_char_boundary(i) {
        i -= 1;
    }
    i
}

/// Wrap `sel` in `open`/`close`. Collapsed cursor inserts an empty pair
/// with the caret parked between; an empty pair under the cursor toggles
/// back off. Returns the new text plus the caret/selection to install.
pub fn wrap_selection(
    text: &str,
    sel: Range<usize>,
    open: &str,
    close: &str,
) -> (String, Range<usize>) {
    let start = clip(text, sel.start);
    let end = clip(text, sel.end).max(start);
    let (a, rest) = text.split_at(start);
    let (mid, c) = rest.split_at(end - start);
    if mid.is_empty() {
        if a.ends_with(open) && c.starts_with(close) {
            let na = &a[..a.len() - open.len()];
            let nc = &c[close.len()..];
            let caret = na.len();
            return (format!("{na}{nc}"), caret..caret);
        }
        let caret = a.len() + open.len();
        return (format!("{a}{open}{close}{c}"), caret..caret);
    }
    let caret = a.len() + open.len() + mid.len() + close.len();
    (
        format!("{a}{open}{mid}{close}{c}"),
        caret..caret,
    )
}

/// Prepend `prefix` to every non-empty line touched by `sel`.
pub fn prefix_lines(text: &str, sel: Range<usize>, prefix: &str) -> (String, Range<usize>) {
    let start = clip(text, sel.start);
    let end = clip(text, sel.end).max(start);
    let line_start = text[..start].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let line_end;
    if let Some(i) = text[end..].find('\n') {
        line_end = end + i;
    } else {
        line_end = text.len();
    }
    let mut out = String::with_capacity(text.len() + prefix.len() * 4);
    out.push_str(&text[..line_start]);
    let mut added = 0;
    for (n, line) in text[line_start..line_end].split('\n').enumerate() {
        if n > 0 {
            out.push('\n');
        }
        if !line.is_empty() {
            out.push_str(prefix);
            added += prefix.len();
        }
        out.push_str(line);
    }
    out.push_str(&text[line_end..]);
    let caret = end + added;
    (out, caret..caret)
}

/// Link-wrap `sel`, or insert a `[…](url)` template at a collapsed cursor.
pub fn wrap_link(text: &str, sel: Range<usize>) -> (String, Range<usize>) {
    let start = clip(text, sel.start);
    let end = clip(text, sel.end).max(start);
    let (a, rest) = text.split_at(start);
    let (mid, c) = rest.split_at(end - start);
    if mid.is_empty() {
        let caret = a.len() + 1;
        return (format!("{a}[](url){c}"), caret..caret);
    }
    let caret = a.len() + mid.len() + 8;
    (format!("{a}[{mid}](url){c}"), caret..caret)
}

/// Splice `snippet` at `sel`, replacing any selected text.
pub fn insert_at(text: &str, sel: Range<usize>, snippet: &str) -> (String, Range<usize>) {
    let start = clip(text, sel.start);
    let end = clip(text, sel.end).max(start);
    let caret = start + snippet.len();
    (
        format!("{}{}{}", &text[..start], snippet, &text[end..]),
        caret..caret,
    )
}
