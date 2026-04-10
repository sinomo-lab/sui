use std::sync::{Mutex, OnceLock};

use sui_core::{Error, Result, Size};

use crate::{
    cache::{TextLayoutCache, TextLayoutCacheKey, TextLayoutCacheSnapshot},
    flatten::FlattenedTextDocument,
    font::{FontContext, ResolvedSpanInput, TextSystemState},
    layout::layout_document,
    model::{TextDocument, TextLayout, TextLayoutRequest, TextMeasurement, TextRun, TextStyle},
    FontRegistry,
};

#[derive(Debug, Default)]
pub struct TextSystem {
    state: OnceLock<std::result::Result<TextSystemState, String>>,
    layout_cache: Mutex<TextLayoutCache>,
}

impl TextSystem {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn measure_text(
        &self,
        text: impl Into<String>,
        style: TextStyle,
        font_registry: &FontRegistry,
    ) -> Result<TextMeasurement> {
        self.measure_document(TextDocument::from_plain_text(text.into(), style), font_registry)
    }

    pub fn measure_document(
        &self,
        document: TextDocument,
        font_registry: &FontRegistry,
    ) -> Result<TextMeasurement> {
        Ok(self
            .layout_document(TextLayoutRequest::new(document), font_registry)?
            .measurement())
    }

    pub fn shape_text(
        &self,
        text: impl Into<String>,
        box_size: Size,
        style: TextStyle,
        font_registry: &FontRegistry,
    ) -> Result<TextLayout> {
        self.layout_document(
            TextLayoutRequest::new(TextDocument::from_plain_text(text.into(), style))
                .with_box_size(box_size),
            font_registry,
        )
    }

    pub fn layout_document(
        &self,
        request: TextLayoutRequest,
        font_registry: &FontRegistry,
    ) -> Result<TextLayout> {
        self.shape_text_internal(request, font_registry)
    }

    pub fn shape_text_run(
        &self,
        run: &TextRun,
        font_registry: &FontRegistry,
    ) -> Result<TextLayout> {
        self.shape_text(
            run.text.clone(),
            run.rect.size,
            run.style.clone(),
            font_registry,
        )
    }

    pub fn layout_cache_snapshot(&self) -> TextLayoutCacheSnapshot {
        self.layout_cache
            .lock()
            .ok()
            .map(|cache| cache.snapshot())
            .unwrap_or_default()
    }

    fn shape_text_internal(
        &self,
        request: TextLayoutRequest,
        font_registry: &FontRegistry,
    ) -> Result<TextLayout> {
        let normalized_document = request.document.normalized();
        let flattened = FlattenedTextDocument::new(normalized_document.clone());
        let font_context = self.font_context(font_registry)?;
        let resolved_spans = self.resolve_span_inputs(&flattened, &font_context)?;
        let span_face_keys = resolved_spans
            .iter()
            .map(|span| span.cache_face_key)
            .collect::<Vec<_>>();
        let cache_key = TextLayoutCacheKey::new(&flattened, &span_face_keys, request.box_size);

        if let Some(cached) = self.cached_layout(&cache_key)? {
            return Ok(cached.with_document(normalized_document));
        }

        let layout = layout_document(flattened, resolved_spans, request.box_size, font_context)?;
        self.store_layout(cache_key, layout.clone())?;
        Ok(layout.with_document(normalized_document))
    }

    fn resolve_span_inputs(
        &self,
        flattened: &FlattenedTextDocument,
        font_context: &FontContext,
    ) -> Result<Vec<ResolvedSpanInput>> {
        flattened
            .spans
            .iter()
            .map(|span| {
                font_context.resolve_span(
                    span.id.clone(),
                    span.text.clone(),
                    &span.style,
                )
            })
            .collect()
    }

    fn cached_layout(&self, key: &TextLayoutCacheKey) -> Result<Option<TextLayout>> {
        let mut cache = self
            .layout_cache
            .lock()
            .map_err(|_| Error::new("text layout cache lock was poisoned"))?;
        Ok(cache.get(key))
    }

    fn store_layout(&self, key: TextLayoutCacheKey, layout: TextLayout) -> Result<()> {
        let mut cache = self
            .layout_cache
            .lock()
            .map_err(|_| Error::new("text layout cache lock was poisoned"))?;
        cache.insert(key, layout);
        Ok(())
    }

    fn font_context(&self, font_registry: &FontRegistry) -> Result<FontContext> {
        let state = self
            .state
            .get_or_init(|| TextSystemState::new().map_err(|error| error.to_string()));
        match state {
            Ok(state) => state.build_font_context(font_registry),
            Err(message) => Err(Error::new(message.clone())),
        }
    }
}