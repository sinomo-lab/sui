use sui_core::{Point, Rect, SemanticsNode, SemanticsRole, SemanticsValue, Size};
use sui_layout::{Constraints, Padding as Insets};
use sui_runtime::{MeasureCtx, PaintCtx, SemanticsCtx, Widget};
use sui_text::{
    PersistentTextLayout, TextDocument, TextLayoutRequest, TextParagraph, TextSpan, TextStyle,
};

use crate::DefaultTheme;

pub struct RichText {
    document: TextDocument,
    document_reader: Option<Box<dyn Fn() -> TextDocument>>,
    semantic_name: Option<String>,
    padding: Insets,
    min_width: f32,
    min_height: f32,
    layout: Option<PersistentTextLayout>,
}

impl RichText {
    pub fn new(document: TextDocument) -> Self {
        Self {
            document,
            document_reader: None,
            semantic_name: None,
            padding: Insets::ZERO,
            min_width: 0.0,
            min_height: 0.0,
            layout: None,
        }
    }

    pub fn dynamic<F>(fallback: TextDocument, reader: F) -> Self
    where
        F: Fn() -> TextDocument + 'static,
    {
        Self::new(fallback).document_when(reader)
    }

    pub fn plain(text: impl Into<String>) -> Self {
        Self::from_plain_text(text, DefaultTheme::default().body_text_style())
    }

    pub fn from_plain_text(text: impl Into<String>, style: TextStyle) -> Self {
        Self::new(TextDocument::from_plain_text(text, style))
    }

    pub fn from_spans(spans: Vec<TextSpan>) -> Self {
        Self::new(TextDocument {
            paragraphs: vec![TextParagraph::from_spans(spans)],
        })
    }

    pub fn document(&self) -> &TextDocument {
        &self.document
    }

    pub fn set_document(&mut self, document: TextDocument) {
        self.document = document;
        self.document_reader = None;
        self.layout = None;
    }

    pub fn document_when<F>(mut self, reader: F) -> Self
    where
        F: Fn() -> TextDocument + 'static,
    {
        self.document_reader = Some(Box::new(reader));
        self
    }

    pub fn semantic_name(mut self, name: impl Into<String>) -> Self {
        self.semantic_name = Some(name.into());
        self
    }

    pub fn padding(mut self, padding: Insets) -> Self {
        self.padding = padding;
        self
    }

    pub fn min_width(mut self, width: f32) -> Self {
        self.min_width = width.max(0.0);
        self
    }

    pub fn min_height(mut self, height: f32) -> Self {
        self.min_height = height.max(0.0);
        self
    }

    fn current_document(&self) -> TextDocument {
        self.document_reader
            .as_ref()
            .map(|reader| reader())
            .unwrap_or_else(|| self.document.clone())
    }

    fn content_constraints(&self, constraints: Constraints) -> Constraints {
        Constraints::new(
            self.padding.inset(constraints.min),
            self.padding.inset(constraints.max),
        )
    }

    fn layout_request(
        &self,
        document: TextDocument,
        content_constraints: Constraints,
    ) -> TextLayoutRequest {
        let max_width = content_constraints.max.width;
        if max_width.is_finite() {
            TextLayoutRequest::new(document).with_box_size(Size::new(max_width.max(1.0), 1.0))
        } else {
            TextLayoutRequest::new(document)
        }
    }

    fn content_rect(&self, bounds: Rect) -> Rect {
        Rect::new(
            bounds.x() + self.padding.left,
            bounds.y() + self.padding.top,
            (bounds.width() - (self.padding.left + self.padding.right)).max(0.0),
            (bounds.height() - (self.padding.top + self.padding.bottom)).max(0.0),
        )
    }

    fn padded_size(&self, content_size: Size) -> Size {
        Size::new(
            (content_size.width + self.padding.left + self.padding.right).max(self.min_width),
            (content_size.height + self.padding.top + self.padding.bottom).max(self.min_height),
        )
    }
}

impl Default for RichText {
    fn default() -> Self {
        Self::plain("")
    }
}

impl Widget for RichText {
    fn measure(&mut self, ctx: &mut MeasureCtx, constraints: Constraints) -> Size {
        let document = self.current_document();
        let content_constraints = self.content_constraints(constraints);
        let request = self.layout_request(document, content_constraints);
        let handle = self.layout.as_ref().map(|layout| layout.handle());
        self.layout = ctx
            .layout()
            .layout_document_persistent(handle, request)
            .ok();

        let content_size = self
            .layout
            .as_ref()
            .map(|layout| {
                let measurement = layout.measurement();
                Size::new(measurement.width.max(0.0), measurement.height.max(0.0))
            })
            .unwrap_or(Size::ZERO);

        constraints.clamp(self.padded_size(content_size))
    }

    fn paint(&self, ctx: &mut PaintCtx) {
        let Some(layout) = &self.layout else {
            return;
        };
        let content = self.content_rect(ctx.bounds());
        let layout_bounds = layout.measurement().bounds;
        let origin = Point::new(content.x() - layout_bounds.x(), content.y());
        ctx.draw_persistent_text_layout(origin, layout);
    }

    fn semantics(&self, ctx: &mut SemanticsCtx) {
        let text = self.current_document().plain_text();
        let mut node = SemanticsNode::new(ctx.widget_id(), SemanticsRole::Text, ctx.bounds());
        node.name = Some(self.semantic_name.clone().unwrap_or_else(|| text.clone()));
        if self.semantic_name.is_some() {
            node.value = Some(SemanticsValue::Text(text));
        }
        ctx.push(node);
    }
}

#[cfg(test)]
mod tests {
    use super::RichText;
    use crate::SizedBox;
    use sui_core::{Color, SemanticsRole, SemanticsValue};
    use sui_runtime::{Application, RenderOutput, Widget, WindowBuilder};
    use sui_scene::SceneCommand;
    use sui_text::{FontStyle, FontWeight, TextDocument, TextParagraph, TextSpan, TextStyle};

    fn render<W>(root: W) -> RenderOutput
    where
        W: Widget + 'static,
    {
        let mut runtime = Application::new()
            .window(WindowBuilder::new().title("Rich text").root(root))
            .build()
            .unwrap();
        let window_id = runtime.window_ids()[0];
        runtime.render(window_id).unwrap()
    }

    fn two_span_document() -> TextDocument {
        let mut strong = TextStyle::new(Color::rgba(0.84, 0.16, 0.18, 1.0));
        strong.weight = FontWeight::BOLD;
        let mut emphasis = TextStyle::new(Color::rgba(0.10, 0.38, 0.82, 1.0));
        emphasis.style = FontStyle::Italic;
        TextDocument {
            paragraphs: vec![TextParagraph::from_spans(vec![
                TextSpan::new("Warm", strong),
                TextSpan::new(" cool", emphasis),
            ])],
        }
    }

    #[test]
    fn rich_text_paints_document_spans_without_color_override() {
        let output = render(RichText::new(two_span_document()));
        let mut shaped = None;
        output.frame.scene.visit_commands(&mut |command| {
            if let SceneCommand::DrawShapedText(text) = command {
                shaped = Some(text.clone());
            }
        });
        let shaped = shaped.expect("rich text should emit shaped text");
        assert_eq!(shaped.color_override, None);

        let layout = shaped
            .resolve(output.frame.text_layout_registry.as_ref())
            .expect("rich text layout should resolve");
        assert_eq!(layout.text(), "Warm cool");
        assert_eq!(layout.runs().len(), 2);
        assert_eq!(layout.run_style(0).weight, FontWeight::BOLD);
        assert_eq!(layout.run_style(1).style, FontStyle::Italic);
        assert_ne!(layout.run_style(0).color, layout.run_style(1).color);
    }

    #[test]
    fn rich_text_exposes_plain_text_semantics_for_named_document() {
        let document = TextDocument {
            paragraphs: vec![
                TextParagraph::new("First", TextStyle::new(Color::WHITE)),
                TextParagraph::new("Second", TextStyle::new(Color::WHITE)),
            ],
        };
        let output = render(RichText::new(document).semantic_name("Summary"));
        let node = output
            .semantics
            .iter()
            .find(|node| node.role == SemanticsRole::Text)
            .expect("rich text should expose text semantics");

        assert_eq!(node.name.as_deref(), Some("Summary"));
        assert_eq!(
            node.value,
            Some(SemanticsValue::Text("First\nSecond".to_string()))
        );
    }

    #[test]
    fn rich_text_wraps_to_parent_constraints() {
        let output = render(
            SizedBox::new()
                .width(96.0)
                .with_child(RichText::from_plain_text(
                    "alpha beta gamma delta epsilon",
                    TextStyle::new(Color::WHITE),
                )),
        );
        let mut line_count = 0;
        output.frame.scene.visit_commands(&mut |command| {
            if let SceneCommand::DrawShapedText(text) = command
                && let Some(layout) = text.resolve(output.frame.text_layout_registry.as_ref())
            {
                line_count = layout.lines().len();
            }
        });

        assert!(line_count > 1, "expected constrained rich text to wrap");
    }
}
