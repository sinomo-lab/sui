use std::{
    cell::RefCell,
    collections::HashMap,
    sync::{Mutex, OnceLock},
    time::Instant,
};

use sui_core::{Error, Result, Size};

use crate::{
    cache::{TextLayoutCache, TextLayoutCacheSnapshot},
    flatten::FlattenedTextDocument,
    font::{FaceCacheKey, FontContext, ResolvedSpanInput, TextSystemState},
    layout::layout_document,
    model::{TextDocument, TextLayout, TextLayoutRequest, TextMeasurement, TextRun, TextStyle},
    FontRegistry,
};

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct RuntimeTextTimingDiagnostics {
    pub request_count: usize,
    pub cache_hit_count: usize,
    pub cache_miss_count: usize,
    pub total_time_us: u64,
    pub prelookup_time_us: u64,
    pub cache_lookup_time_us: u64,
    pub miss_layout_time_us: u64,
}

thread_local! {
    static TEXT_TIMING_COLLECTOR: RefCell<Option<RuntimeTextTimingDiagnostics>> =
        const { RefCell::new(None) };
}

fn text_timing_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var_os("SUI_PROFILE_TEXT_TIMINGS").is_some())
}

pub fn begin_text_timing_collection() {
    if !text_timing_enabled() {
        return;
    }

    TEXT_TIMING_COLLECTOR.with(|collector| {
        *collector.borrow_mut() = Some(RuntimeTextTimingDiagnostics::default());
    });
}

pub fn take_text_timing_collection() -> RuntimeTextTimingDiagnostics {
    if !text_timing_enabled() {
        return RuntimeTextTimingDiagnostics::default();
    }

    TEXT_TIMING_COLLECTOR
        .with(|collector| collector.borrow_mut().take())
        .unwrap_or_default()
}

fn record_text_timing(
    total_time_us: u64,
    prelookup_time_us: u64,
    cache_lookup_time_us: u64,
    miss_layout_time_us: u64,
    cache_hit: bool,
) {
    if !text_timing_enabled() {
        return;
    }

    TEXT_TIMING_COLLECTOR.with(|collector| {
        let mut collector = collector.borrow_mut();
        let Some(diagnostics) = collector.as_mut() else {
            return;
        };
        diagnostics.request_count += 1;
        diagnostics.total_time_us += total_time_us;
        diagnostics.prelookup_time_us += prelookup_time_us;
        diagnostics.cache_lookup_time_us += cache_lookup_time_us;
        diagnostics.miss_layout_time_us += miss_layout_time_us;
        if cache_hit {
            diagnostics.cache_hit_count += 1;
        } else {
            diagnostics.cache_miss_count += 1;
        }
    });
}

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
        let total_started = text_timing_enabled().then(Instant::now);
        let TextLayoutRequest { document, box_size } = request;
        let normalized_document = document.into_normalized();
        let span_face_keys = self.resolve_span_face_keys(&normalized_document, font_registry)?;
        let prelookup_time_us = total_started
            .as_ref()
            .map(|started| started.elapsed().as_micros() as u64)
            .unwrap_or(0);

        let lookup_started = text_timing_enabled().then(Instant::now);
        if let Some(cached) = self.cached_layout(&normalized_document, &span_face_keys, box_size)? {
            let cache_lookup_time_us = lookup_started
                .as_ref()
                .map(|started| started.elapsed().as_micros() as u64)
                .unwrap_or(0);
            let total_time_us = total_started
                .as_ref()
                .map(|started| started.elapsed().as_micros() as u64)
                .unwrap_or(0);
            record_text_timing(total_time_us, prelookup_time_us, cache_lookup_time_us, 0, true);
            return Ok(cached.with_document(normalized_document));
        }

        let cache_lookup_time_us = lookup_started
            .as_ref()
            .map(|started| started.elapsed().as_micros() as u64)
            .unwrap_or(0);

        let layout_started = text_timing_enabled().then(Instant::now);
        let flattened = FlattenedTextDocument::new(normalized_document.clone());
        let font_context = self.font_context(font_registry)?;
        let resolved_spans = self.resolve_span_inputs(&flattened, &font_context)?;
        let layout_id = crate::cache::TextLayoutCacheKey::stable_layout_id(
            &normalized_document,
            &span_face_keys,
            box_size,
        );
        let layout = layout_document(flattened, resolved_spans, box_size, font_context, layout_id)?;
        let miss_layout_time_us = layout_started
            .as_ref()
            .map(|started| started.elapsed().as_micros() as u64)
            .unwrap_or(0);
        self.store_layout(&normalized_document, &span_face_keys, box_size, layout.clone())?;
        let total_time_us = total_started
            .as_ref()
            .map(|started| started.elapsed().as_micros() as u64)
            .unwrap_or(0);
        record_text_timing(
            total_time_us,
            prelookup_time_us,
            cache_lookup_time_us,
            miss_layout_time_us,
            false,
        );
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

    fn resolve_span_face_keys(
        &self,
        document: &TextDocument,
        font_registry: &FontRegistry,
    ) -> Result<Vec<FaceCacheKey>> {
        let state = self.text_system_state()?;
        let default_face_key = state.default_face_key();
        let total_spans = document
            .paragraphs
            .iter()
            .map(|paragraph| paragraph.spans.len())
            .sum();
        let mut keys = Vec::with_capacity(total_spans);
        let mut explicit_face_keys = HashMap::new();

        for paragraph in &document.paragraphs {
            for span in &paragraph.spans {
                let face_key = match span.style.font {
                    Some(handle) => {
                        if let Some(face_key) = explicit_face_keys.get(&handle).copied() {
                            face_key
                        } else {
                            let font = font_registry.get(handle).ok_or_else(|| {
                                Error::new(format!("font handle {} is not registered", handle.get()))
                            })?;
                            let face_key = FaceCacheKey::from_registered_font(font);
                            explicit_face_keys.insert(handle, face_key);
                            face_key
                        }
                    }
                    None => default_face_key,
                };
                keys.push(face_key);
            }
        }

        Ok(keys)
    }

    fn cached_layout(
        &self,
        document: &TextDocument,
        span_face_keys: &[FaceCacheKey],
        box_size: Option<Size>,
    ) -> Result<Option<TextLayout>> {
        let mut cache = self
            .layout_cache
            .lock()
            .map_err(|_| Error::new("text layout cache lock was poisoned"))?;
        Ok(cache.get(document, span_face_keys, box_size))
    }

    fn store_layout(
        &self,
        document: &TextDocument,
        span_face_keys: &[FaceCacheKey],
        box_size: Option<Size>,
        layout: TextLayout,
    ) -> Result<()> {
        let mut cache = self
            .layout_cache
            .lock()
            .map_err(|_| Error::new("text layout cache lock was poisoned"))?;
        cache.insert(document, span_face_keys, box_size, layout);
        Ok(())
    }

    fn text_system_state(&self) -> Result<&TextSystemState> {
        let state = self
            .state
            .get_or_init(|| TextSystemState::new().map_err(|error| error.to_string()));
        match state {
            Ok(state) => Ok(state),
            Err(message) => Err(Error::new(message.clone())),
        }
    }

    fn font_context(&self, font_registry: &FontRegistry) -> Result<FontContext> {
        self.text_system_state()?.build_font_context(font_registry)
    }
}