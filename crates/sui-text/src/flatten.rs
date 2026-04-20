use std::ops::Range;

use crate::model::{TextDocument, TextParagraphStyle, TextSpanId, TextStyle};

#[derive(Debug, Clone)]
pub(crate) struct FlattenedTextDocument {
    pub document: TextDocument,
    pub text: String,
    pub paragraphs: Vec<FlattenedParagraph>,
    pub spans: Vec<FlattenedSpan>,
}

impl FlattenedTextDocument {
    pub(crate) fn new(document: TextDocument) -> Self {
        let mut text = String::new();
        let mut paragraphs = Vec::with_capacity(document.paragraphs.len());
        let mut spans = Vec::new();

        for (paragraph_index, paragraph) in document.paragraphs.iter().enumerate() {
            let paragraph_start = text.len();
            let span_start = spans.len();
            for (span_index, span) in paragraph.spans.iter().enumerate() {
                text.push_str(&span.text);
                spans.push(FlattenedSpan {
                    id: TextSpanId {
                        paragraph_index,
                        span_index,
                    },
                    text: span.text.clone(),
                    style: span.style.clone(),
                });
            }

            paragraphs.push(FlattenedParagraph {
                index: paragraph_index,
                byte_range: paragraph_start..text.len(),
                style: paragraph.style.clone(),
                span_range: span_start..spans.len(),
            });

            if paragraph_index + 1 < document.paragraphs.len() {
                text.push('\n');
            }
        }

        Self {
            document,
            text,
            paragraphs,
            spans,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct FlattenedParagraph {
    pub index: usize,
    pub byte_range: Range<usize>,
    pub style: TextParagraphStyle,
    pub span_range: Range<usize>,
}

#[derive(Debug, Clone)]
pub(crate) struct FlattenedSpan {
    pub id: TextSpanId,
    pub text: String,
    pub style: TextStyle,
}
