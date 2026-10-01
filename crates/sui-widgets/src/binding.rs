//! A widget property that is fixed, read when needed, or observed, as the
//! `p`, `p_when`, and `p_from` builders set it.

use std::sync::Arc;

use sui_reactive::Observable;

pub(crate) struct Binding<T> {
    value: T,
    reader: Option<Box<dyn Fn() -> T>>,
    source: Option<Arc<dyn Observable<T>>>,
}

impl<T: Clone + 'static> Binding<T> {
    pub(crate) fn new(value: T) -> Self {
        Self {
            value,
            reader: None,
            source: None,
        }
    }

    /// Fix the value, dropping any reader or observable.
    pub(crate) fn set(&mut self, value: T) {
        self.value = value;
        self.reader = None;
        self.source = None;
    }

    /// Read the value from `reader` whenever it is needed.
    pub(crate) fn set_when(&mut self, reader: impl Fn() -> T + 'static) {
        self.reader = Some(Box::new(reader));
        self.source = None;
    }

    /// Follow `source`, which the widget subscribes to through [`Self::observe`].
    pub(crate) fn set_from(&mut self, source: impl Observable<T> + 'static) {
        self.source = Some(Arc::new(source));
        self.reader = None;
    }

    pub(crate) fn get(&self) -> T {
        if let Some(source) = &self.source {
            return source.get();
        }
        self.reader
            .as_ref()
            .map_or_else(|| self.value.clone(), |reader| reader())
    }

    /// Subscribe through `observe`, such as a context's `observe`, when the
    /// value is observed, so the widget repaints when it changes.
    pub(crate) fn observe(&self, observe: impl FnOnce(&dyn Observable<T>)) {
        if let Some(source) = &self.source {
            observe(source.as_ref());
        }
    }

    /// Whether the value can change without the widget being told, which
    /// rules out reusing the widget's last output.
    pub(crate) fn is_live(&self) -> bool {
        self.reader.is_some() || self.source.is_some()
    }
}
