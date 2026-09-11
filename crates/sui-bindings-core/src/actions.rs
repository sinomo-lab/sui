use crate::errors::ForeignCallbackResult;
use std::fmt;
use std::sync::Arc;
use sui::Color;

#[derive(Clone)]
pub struct BindingAction {
    pub(crate) callback: Arc<dyn Fn() -> ForeignCallbackResult<()> + Send + Sync + 'static>,
}

impl BindingAction {
    pub fn new(callback: impl Fn() -> ForeignCallbackResult<()> + Send + Sync + 'static) -> Self {
        Self {
            callback: Arc::new(callback),
        }
    }

    pub fn run(&self) -> ForeignCallbackResult<()> {
        (self.callback)()
    }
}

impl fmt::Debug for BindingAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BindingAction").finish_non_exhaustive()
    }
}

#[derive(Clone)]
pub struct BindingBoolAction {
    pub(crate) callback: Arc<dyn Fn(bool) -> ForeignCallbackResult<()> + Send + Sync + 'static>,
}

impl BindingBoolAction {
    pub fn new(
        callback: impl Fn(bool) -> ForeignCallbackResult<()> + Send + Sync + 'static,
    ) -> Self {
        Self {
            callback: Arc::new(callback),
        }
    }

    pub fn run(&self, value: bool) -> ForeignCallbackResult<()> {
        (self.callback)(value)
    }
}

impl fmt::Debug for BindingBoolAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BindingBoolAction").finish_non_exhaustive()
    }
}

#[derive(Clone)]
pub struct BindingNumberAction {
    pub(crate) callback: Arc<dyn Fn(f64) -> ForeignCallbackResult<()> + Send + Sync + 'static>,
}

#[derive(Clone)]
pub struct BindingIdAction {
    pub(crate) callback: Arc<dyn Fn(u64) -> ForeignCallbackResult<()> + Send + Sync + 'static>,
}

impl BindingIdAction {
    pub fn new(
        callback: impl Fn(u64) -> ForeignCallbackResult<()> + Send + Sync + 'static,
    ) -> Self {
        Self {
            callback: Arc::new(callback),
        }
    }

    pub fn run(&self, value: u64) -> ForeignCallbackResult<()> {
        (self.callback)(value)
    }
}

impl fmt::Debug for BindingIdAction {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BindingIdAction")
            .finish_non_exhaustive()
    }
}

impl BindingNumberAction {
    pub fn new(
        callback: impl Fn(f64) -> ForeignCallbackResult<()> + Send + Sync + 'static,
    ) -> Self {
        Self {
            callback: Arc::new(callback),
        }
    }

    pub fn run(&self, value: f64) -> ForeignCallbackResult<()> {
        (self.callback)(value)
    }
}

impl fmt::Debug for BindingNumberAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BindingNumberAction")
            .finish_non_exhaustive()
    }
}

#[derive(Clone)]
pub struct BindingReorderAction {
    pub(crate) callback:
        Arc<dyn Fn(usize, usize, usize) -> ForeignCallbackResult<()> + Send + Sync + 'static>,
}

impl BindingReorderAction {
    pub fn new(
        callback: impl Fn(usize, usize, usize) -> ForeignCallbackResult<()> + Send + Sync + 'static,
    ) -> Self {
        Self {
            callback: Arc::new(callback),
        }
    }

    pub fn run(&self, item: usize, from: usize, to: usize) -> ForeignCallbackResult<()> {
        (self.callback)(item, from, to)
    }
}

impl fmt::Debug for BindingReorderAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BindingReorderAction")
            .finish_non_exhaustive()
    }
}

#[derive(Clone)]
pub struct BindingStringAction {
    pub(crate) callback: Arc<dyn Fn(String) -> ForeignCallbackResult<()> + Send + Sync + 'static>,
}

#[derive(Clone)]
pub struct BindingStringsAction {
    pub(crate) callback:
        Arc<dyn Fn(Vec<String>) -> ForeignCallbackResult<()> + Send + Sync + 'static>,
}

impl BindingStringsAction {
    pub fn new(
        callback: impl Fn(Vec<String>) -> ForeignCallbackResult<()> + Send + Sync + 'static,
    ) -> Self {
        Self {
            callback: Arc::new(callback),
        }
    }

    pub fn run(&self, values: Vec<String>) -> ForeignCallbackResult<()> {
        (self.callback)(values)
    }
}

impl fmt::Debug for BindingStringsAction {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BindingStringsAction")
            .finish_non_exhaustive()
    }
}

impl BindingStringAction {
    pub fn new(
        callback: impl Fn(String) -> ForeignCallbackResult<()> + Send + Sync + 'static,
    ) -> Self {
        Self {
            callback: Arc::new(callback),
        }
    }

    pub fn run(&self, value: String) -> ForeignCallbackResult<()> {
        (self.callback)(value)
    }
}

impl fmt::Debug for BindingStringAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BindingStringAction")
            .finish_non_exhaustive()
    }
}

#[derive(Clone)]
pub struct BindingSelectAction {
    pub(crate) callback:
        Arc<dyn Fn(usize, String) -> ForeignCallbackResult<()> + Send + Sync + 'static>,
}

impl BindingSelectAction {
    pub fn new(
        callback: impl Fn(usize, String) -> ForeignCallbackResult<()> + Send + Sync + 'static,
    ) -> Self {
        Self {
            callback: Arc::new(callback),
        }
    }

    pub fn run(&self, index: usize, value: String) -> ForeignCallbackResult<()> {
        (self.callback)(index, value)
    }
}

impl fmt::Debug for BindingSelectAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BindingSelectAction")
            .finish_non_exhaustive()
    }
}

#[derive(Clone)]
pub struct BindingColorAction {
    pub(crate) callback: Arc<dyn Fn(Color) -> ForeignCallbackResult<()> + Send + Sync + 'static>,
}

impl BindingColorAction {
    pub fn new(
        callback: impl Fn(Color) -> ForeignCallbackResult<()> + Send + Sync + 'static,
    ) -> Self {
        Self {
            callback: Arc::new(callback),
        }
    }

    pub fn run(&self, color: Color) -> ForeignCallbackResult<()> {
        (self.callback)(color)
    }
}

impl fmt::Debug for BindingColorAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BindingColorAction").finish_non_exhaustive()
    }
}

#[derive(Clone)]
pub struct BindingColorSelectAction {
    pub(crate) callback:
        Arc<dyn Fn(usize, String, Color) -> ForeignCallbackResult<()> + Send + Sync + 'static>,
}

impl BindingColorSelectAction {
    pub fn new(
        callback: impl Fn(usize, String, Color) -> ForeignCallbackResult<()> + Send + Sync + 'static,
    ) -> Self {
        Self {
            callback: Arc::new(callback),
        }
    }

    pub fn run(&self, index: usize, name: String, color: Color) -> ForeignCallbackResult<()> {
        (self.callback)(index, name, color)
    }
}

impl fmt::Debug for BindingColorSelectAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BindingColorSelectAction")
            .finish_non_exhaustive()
    }
}
