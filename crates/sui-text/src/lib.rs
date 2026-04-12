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
    TextDirection, TextDocument, TextFlowDirection, TextGlyphInstance, TextLayout,
    TextLayoutId, TextLayoutMetadata, TextLayoutRequest, TextLayoutRun, TextLayoutVersion,
    TextLayoutView, TextLine, TextLineWindow, TextMeasurement, TextParagraph,
    TextParagraphLayout, TextParagraphStyle, TextRun, TextRunView, TextSelection,
    TextSelectionGeometry, TextSpan, TextSpanId, TextStyle, TextWrap, TextWritingMode,
};
pub use system::{
    RuntimeTextTimingDiagnostics, TextSystem, begin_text_timing_collection,
    take_text_timing_collection,
};

#[cfg(test)]
mod tests;