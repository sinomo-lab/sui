//! `Widget`: custom widgets with JavaScript callbacks, and the descriptors
//! that factories return.

use crate::prelude::*;

#[wasm_bindgen(js_name = Widget)]
pub struct JsWidget {
    pub(crate) kind: JsWidgetKind,
}

js_class!(JsWidget => "Widget");

pub(crate) enum JsWidgetKind {
    Foreign {
        callbacks: Mutex<Option<JsObjectCallbacks>>,
        children: Vec<BindingWidget>,
    },
    Binding(BindingWidget),
}

#[wasm_bindgen(js_class = Widget)]
impl JsWidget {
    #[wasm_bindgen(constructor)]
    pub fn new(callbacks: Object, children: Option<Array>) -> Result<Self> {
        Ok(Self {
            kind: JsWidgetKind::Foreign {
                callbacks: Mutex::new(Some(JsObjectCallbacks::new(callbacks)?)),
                children: extract_js_widget_children(children.as_ref())?,
            },
        })
    }
}

impl JsWidget {
    pub(crate) fn from_binding(widget: BindingWidget) -> Self {
        Self {
            kind: JsWidgetKind::Binding(widget),
        }
    }

    /// Apply a factory's `semanticName` argument.
    pub(crate) fn with_semantic_name(self, name: Option<String>) -> Result<Self> {
        let Some(name) = name else {
            return Ok(self);
        };
        Ok(Self::from_binding(
            self.binding_widget()?.with_semantic_name(name),
        ))
    }

    /// Apply a factory's `enabled` argument: a boolean or a `State`.
    pub(crate) fn with_enabled(self, enabled: Option<JsBindingBoolArg>) -> Result<Self> {
        let Some(enabled) = enabled else {
            return Ok(self);
        };
        Ok(Self::from_binding(
            self.binding_widget()?
                .with_enabled(binding_bool_from_js(enabled)?),
        ))
    }

    pub(crate) fn binding_widget(&self) -> Result<BindingWidget> {
        match &self.kind {
            JsWidgetKind::Binding(widget) => Ok(widget.clone()),
            JsWidgetKind::Foreign {
                callbacks,
                children,
            } => {
                let callbacks = recover_lock(callbacks)
                    .as_ref()
                    .ok_or_else(|| runtime_error("JavaScript widget callbacks were released"))?
                    .clone_ref()?;
                Ok(BindingWidget::foreign_arc_with_children(
                    Arc::new(JsWidgetCallbacks {
                        callbacks: Mutex::new(Some(callbacks)),
                        child_sizes: Mutex::new(Vec::new()),
                    }),
                    children.clone(),
                ))
            }
        }
    }

    pub(crate) fn into_sui_widget(&self) -> Result<ForeignWidget> {
        match &self.kind {
            JsWidgetKind::Foreign { callbacks, .. } => {
                let callbacks = recover_lock(callbacks)
                    .as_ref()
                    .ok_or_else(|| runtime_error("JavaScript widget callbacks were released"))?
                    .clone_ref()?;
                Ok(ForeignWidget::new(JsWidgetCallbacks {
                    callbacks: Mutex::new(Some(callbacks)),
                    child_sizes: Mutex::new(Vec::new()),
                }))
            }
            JsWidgetKind::Binding(_) => Err(invalid_arg(
                "declarative JavaScript widgets are rendered through BindingApp",
            )),
        }
    }
}

pub(crate) fn extract_js_widget_children(children: Option<&Array>) -> Result<Vec<BindingWidget>> {
    children.map_or_else(|| Ok(Vec::new()), extract_binding_widgets)
}
