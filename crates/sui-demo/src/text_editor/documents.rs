//! The documents the editor opens, and the highlighting for code.

use std::ops::Range;

use sui::prelude::*;
use sui::{TextStyle, TextSurfaceStyleSpan};

/// A document the editor can open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Document {
    Code,
    MixedDirections,
    InputMethods,
    Large,
}

pub(crate) const DOCUMENTS: [Document; 4] = [
    Document::Code,
    Document::MixedDirections,
    Document::InputMethods,
    Document::Large,
];

/// Lines in the large document.
pub(crate) const LARGE_LINES: usize = 20_000;

impl Document {
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Code => "Code",
            Self::MixedDirections => "Mixed-direction prose",
            Self::InputMethods => "Input method practice",
            Self::Large => "Large document (20,000 lines)",
        }
    }

    /// Whether lines wrap when the document opens.
    pub(crate) const fn wraps(self) -> bool {
        matches!(self, Self::MixedDirections | Self::InputMethods)
    }

    pub(crate) const fn highlights(self) -> bool {
        matches!(self, Self::Code)
    }

    pub(crate) fn text(self) -> String {
        match self {
            Self::Code => code(),
            Self::MixedDirections => MIXED_DIRECTIONS.to_string(),
            Self::InputMethods => INPUT_METHODS.to_string(),
            Self::Large => large(),
        }
    }
}

const CODE_HEAD: &str = r#"//! A small layout cache, to type into. Keywords, strings, numbers, and
//! comments are highlighted as you edit.

use std::collections::HashMap;

/// Where a line of text starts and how wide it is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LineMetrics {
    pub start: usize,
    pub width: f32,
}
"#;

const CODE_BLOCK: &str = r#"
/// Lines laid out for width {n}, reused until the text changes.
pub struct Cache{n} {
    lines: HashMap<usize, LineMetrics>,
    revision: u64,
}

impl Cache{n} {
    pub fn get(&mut self, index: usize, revision: u64) -> Option<LineMetrics> {
        if revision != self.revision {
            // The text changed: forget every line. Mixed text: abc שלום 123 مرحبا
            self.lines.clear();
            self.revision = revision;
            return None;
        }
        let width = {n} as f32 * 0.5 + 12.25;
        match self.lines.get(&index) {
            Some(line) if line.width <= width => Some(*line),
            _ => None, // 候補 fallback: Ж 中 नमस्ते 🙂
        }
    }

    pub fn describe(&self) -> String {
        format!("cache {n}: {} lines at revision {}", self.lines.len(), self.revision)
    }
}
"#;

fn code() -> String {
    let mut text = CODE_HEAD.to_string();
    for block in 1..=12 {
        text.push_str(&CODE_BLOCK.replace("{n}", &(block * 40).to_string()));
    }
    text
}

const MIXED_DIRECTIONS: &str = "Mixed-direction prose\n\nThe order number 1024 comes from the Tel Aviv office, where the team wrote שלום עולם on the whiteboard before the meeting. In Cairo, the reply started with مرحبا بالعالم and ended with a date, 2026-09-29.\n\nMove the caret with the arrow keys across each boundary: it moves in reading order, so it jumps when the direction changes. Select across a Hebrew phrase and an English one and notice that the highlight can split into two pieces on one line.\n\nעברית עם מספרים: 3.14 ו-2718 נשארים משמאל לימין בתוך הטקסט.\nالعربية مع أرقام: الرقم 42 يبقى من اليسار إلى اليمين. 🙂\n\nSwitch the direction above to right to left: paragraphs then start at the right edge, and English phrases become the embedded runs.";

const INPUT_METHODS: &str = "Input method practice\n\nTurn on a Japanese, Chinese, or Korean input method and type below. While you compose, the underlined text is not yet part of the document: the inspector shows it under Input method, and the caret and selection stay where the composition started.\n\n日本語：きょうはいいてんきですね。→ 今日はいい天気ですね。\n中文：nihao shijie → 你好世界\n한국어: 안녕하세요 세계\n\nType here: ";

fn large() -> String {
    const WORDS: [&str; 12] = [
        "retained",
        "layout",
        "glyph",
        "atlas",
        "scroll",
        "caret",
        "selection",
        "wrap",
        "cluster",
        "baseline",
        "viewport",
        "window",
    ];
    let mut text = String::with_capacity(LARGE_LINES * 64);
    for line in 1..=LARGE_LINES {
        let a = WORDS[line % WORDS.len()];
        let b = WORDS[(line * 7) % WORDS.len()];
        let c = WORDS[(line * 11) % WORDS.len()];
        text.push_str(&format!(
            "{line:05}  The {a} keeps its {b} while the {c} moves on screen, line {line}.\n"
        ));
    }
    text
}

const KEYWORDS: &[&str] = &[
    "as", "break", "const", "continue", "else", "enum", "false", "fn", "for", "if", "impl", "in",
    "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return", "self", "Self", "struct",
    "trait", "true", "use", "where", "while",
];

fn is_identifier(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// Spans coloring `text` as Rust: keywords, type names, strings, numbers,
/// and comments, drawn with `base` in the theme's colors.
pub(crate) fn highlight(
    text: &str,
    base: &TextStyle,
    theme: DefaultTheme,
) -> Vec<TextSurfaceStyleSpan> {
    let palette = theme.palette;
    let colored = |range: Range<usize>, color: Color| TextSurfaceStyleSpan {
        range,
        style: TextStyle {
            color,
            ..base.clone()
        },
    };
    let bytes = text.as_bytes();
    let mut spans = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        if byte == b'/' && bytes.get(index + 1) == Some(&b'/') {
            let end = text[index..]
                .find('\n')
                .map_or(text.len(), |offset| index + offset);
            spans.push(colored(index..end, palette.text_muted));
            index = end;
        } else if byte == b'"' {
            let mut end = index + 1;
            while end < bytes.len() && bytes[end] != b'"' && bytes[end] != b'\n' {
                end += if bytes[end] == b'\\' { 2 } else { 1 };
            }
            let end = (end + 1).min(bytes.len());
            spans.push(colored(index..end, palette.success));
            index = end;
        } else if byte.is_ascii_digit() && (index == 0 || !is_identifier(bytes[index - 1])) {
            let mut end = index;
            while end < bytes.len() && (is_identifier(bytes[end]) || bytes[end] == b'.') {
                end += 1;
            }
            spans.push(colored(index..end, palette.warning));
            index = end;
        } else if byte.is_ascii_alphabetic() || byte == b'_' {
            let mut end = index;
            while end < bytes.len() && is_identifier(bytes[end]) {
                end += 1;
            }
            let word = &text[index..end];
            if KEYWORDS.contains(&word) {
                spans.push(colored(index..end, palette.accent));
            } else if byte.is_ascii_uppercase() {
                spans.push(colored(index..end, palette.info));
            }
            index = end;
        } else {
            index += 1;
        }
    }
    spans
}
