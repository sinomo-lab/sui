#![forbid(unsafe_code)]

mod cache;
mod flatten;
mod font;
mod layout;
mod model;
mod system;

pub use cache::TextLayoutCacheSnapshot;
pub use font::{FontRegistry, RegisteredFont, ResolvedTextFace};
pub use model::{
    ShapedGlyph, ShapedText, TextAffinity, TextAlign, TextCaret, TextCluster, TextCursor,
    TextDirection, TextDocument, TextFlowDirection, TextLayout, TextLayoutRequest,
    TextLayoutRun, TextLine, TextMeasurement, TextParagraph, TextParagraphLayout,
    TextParagraphStyle, TextRun, TextSelection, TextSelectionGeometry, TextSpan, TextSpanId,
    TextStyle, TextWrap, TextWritingMode,
};
pub use system::TextSystem;

#[cfg(test)]
mod tests;