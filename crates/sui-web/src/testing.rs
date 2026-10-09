//! Semantic queries and render snapshots for tests.

use crate::prelude::*;

js_object! {
    pub struct JsSemanticsQuery {
        role: Option<String> = "role",
        name: Option<String> = "name",
        text: Option<String> = "text",
        description: Option<String> = "description",
        focused: Option<bool> = "focused",
        visible: Option<bool> = "visible",
        /// Only descendants of the node with this id.
        within: Option<String> = "within",
    }
}

impl JsSemanticsQuery {
    pub(crate) fn into_binding(
        self,
        default_visible: Option<bool>,
    ) -> Result<BindingSemanticQuery> {
        let within = self
            .within
            .map(|id| {
                id.parse::<u64>()
                    .map_err(|_| invalid_arg(format!("invalid semantic node id '{id}'")))
            })
            .transpose()?;
        Ok(BindingSemanticQuery {
            role: self.role,
            name: self.name,
            text: self.text,
            description: self.description,
            focused: self.focused,
            visible: self.visible.or(default_visible),
            within,
        })
    }
}

#[wasm_bindgen(js_name = SemanticNode)]
#[derive(Debug, Clone)]
pub struct JsSemanticNode {
    pub(crate) inner: BindingSemanticNode,
}

js_class!(JsSemanticNode => "SemanticNode");

#[wasm_bindgen(js_class = SemanticNode)]
impl JsSemanticNode {
    #[wasm_bindgen(getter = id)]
    pub fn id(&self) -> String {
        self.inner.id.to_string()
    }
    #[wasm_bindgen(getter = parentId)]
    pub fn parent_id(&self) -> Option<String> {
        self.inner.parent_id.map(|id| id.to_string())
    }
    #[wasm_bindgen(getter = role)]
    pub fn role(&self) -> String {
        self.inner.role.clone()
    }
    #[wasm_bindgen(getter = name)]
    pub fn name(&self) -> Option<String> {
        self.inner.name.clone()
    }
    #[wasm_bindgen(getter = value)]
    pub fn value(&self) -> Option<String> {
        self.inner.value.clone()
    }
    #[wasm_bindgen(getter = description)]
    pub fn description(&self) -> Option<String> {
        self.inner.description.clone()
    }
    #[wasm_bindgen(getter = bounds)]
    pub fn bounds(&self) -> JsRect {
        JsRect::new(
            f64::from(self.inner.x),
            f64::from(self.inner.y),
            f64::from(self.inner.width),
            f64::from(self.inner.height),
        )
    }
    #[wasm_bindgen(getter = center)]
    pub fn center(&self) -> JsPoint {
        self.inner.center().into()
    }
    #[wasm_bindgen(getter = actions)]
    pub fn actions(&self) -> Vec<String> {
        self.inner.actions.clone()
    }
    #[wasm_bindgen(getter = checked)]
    pub fn checked(&self) -> Option<String> {
        self.inner.checked.clone()
    }
    #[wasm_bindgen(getter = busy)]
    pub fn busy(&self) -> bool {
        self.inner.busy
    }
    #[wasm_bindgen(getter = disabled)]
    pub fn disabled(&self) -> bool {
        self.inner.disabled
    }
    #[wasm_bindgen(getter = focused)]
    pub fn focused(&self) -> bool {
        self.inner.focused
    }
    #[wasm_bindgen(getter = hidden)]
    pub fn hidden(&self) -> bool {
        self.inner.hidden
    }
    #[wasm_bindgen(getter = hovered)]
    pub fn hovered(&self) -> bool {
        self.inner.hovered
    }
    #[wasm_bindgen(getter = selected)]
    pub fn selected(&self) -> bool {
        self.inner.selected
    }
    #[wasm_bindgen(getter = expanded)]
    pub fn expanded(&self) -> Option<bool> {
        self.inner.expanded
    }
    #[wasm_bindgen(getter = editable)]
    pub fn editable(&self) -> bool {
        self.inner.editable
    }
    #[wasm_bindgen(getter = multiline)]
    pub fn multiline(&self) -> bool {
        self.inner.multiline
    }
    #[wasm_bindgen(getter = visible)]
    pub fn visible(&self) -> bool {
        self.inner.visible()
    }
}

impl From<BindingSemanticNode> for JsSemanticNode {
    fn from(inner: BindingSemanticNode) -> Self {
        Self { inner }
    }
}

#[wasm_bindgen(js_name = RenderSnapshot)]
#[derive(Debug, Clone)]
pub struct JsRenderSnapshot {
    pub(crate) command_count: u32,
    pub(crate) semantics_count: u32,
    pub(crate) semantics_nodes: Vec<JsSemanticNode>,
    pub(crate) semantics_roles: Vec<String>,
    pub(crate) semantics_names: Vec<String>,
    pub(crate) semantics_values: Vec<String>,
    pub(crate) semantics_descriptions: Vec<String>,
    pub(crate) semantics_checked: Vec<String>,
    pub(crate) semantics_busy: Vec<bool>,
    pub(crate) semantics_editable_multiline: Vec<bool>,
    pub(crate) semantics_disabled: Vec<bool>,
    pub(crate) semantics_focused: Vec<bool>,
    pub(crate) semantics_hidden: Vec<bool>,
    pub(crate) semantics_hovered: Vec<bool>,
    pub(crate) semantics_selected: Vec<bool>,
    pub(crate) semantics_expanded: Vec<String>,
    pub(crate) fill_rect_count: u32,
    pub(crate) draw_image_count: u32,
    pub(crate) registered_font_count: u32,
    pub(crate) registered_image_count: u32,
}

js_class!(JsRenderSnapshot => "RenderSnapshot");

#[wasm_bindgen(js_class = RenderSnapshot)]
impl JsRenderSnapshot {
    #[wasm_bindgen(getter = commandCount)]
    pub fn command_count(&self) -> u32 {
        self.command_count
    }

    #[wasm_bindgen(getter = semanticsCount)]
    pub fn semantics_count(&self) -> u32 {
        self.semantics_count
    }

    #[wasm_bindgen(getter = semanticsRoles)]
    pub fn semantics_roles(&self) -> JsValue {
        self.semantics_roles.clone().into_js()
    }

    #[wasm_bindgen(getter = semanticsNames)]
    pub fn semantics_names(&self) -> JsValue {
        self.semantics_names.clone().into_js()
    }

    #[wasm_bindgen(getter = semanticsValues)]
    pub fn semantics_values(&self) -> JsValue {
        self.semantics_values.clone().into_js()
    }

    #[wasm_bindgen(getter = semanticsDescriptions)]
    pub fn semantics_descriptions(&self) -> JsValue {
        self.semantics_descriptions.clone().into_js()
    }

    #[wasm_bindgen(getter = semanticsChecked)]
    pub fn semantics_checked(&self) -> JsValue {
        self.semantics_checked.clone().into_js()
    }

    #[wasm_bindgen(getter = semanticsBusy)]
    pub fn semantics_busy(&self) -> JsValue {
        self.semantics_busy.clone().into_js()
    }

    #[wasm_bindgen(getter = semanticsEditableMultiline)]
    pub fn semantics_editable_multiline(&self) -> JsValue {
        self.semantics_editable_multiline.clone().into_js()
    }

    #[wasm_bindgen(getter = semanticsDisabled)]
    pub fn semantics_disabled(&self) -> JsValue {
        self.semantics_disabled.clone().into_js()
    }

    #[wasm_bindgen(getter = semanticsFocused)]
    pub fn semantics_focused(&self) -> JsValue {
        self.semantics_focused.clone().into_js()
    }

    #[wasm_bindgen(getter = semanticsHidden)]
    pub fn semantics_hidden(&self) -> JsValue {
        self.semantics_hidden.clone().into_js()
    }

    #[wasm_bindgen(getter = semanticsHovered)]
    pub fn semantics_hovered(&self) -> JsValue {
        self.semantics_hovered.clone().into_js()
    }

    #[wasm_bindgen(getter = semanticsSelected)]
    pub fn semantics_selected(&self) -> JsValue {
        self.semantics_selected.clone().into_js()
    }

    #[wasm_bindgen(getter = semanticsExpanded)]
    pub fn semantics_expanded(&self) -> JsValue {
        self.semantics_expanded.clone().into_js()
    }

    #[wasm_bindgen(getter = fillRectCount)]
    pub fn fill_rect_count(&self) -> u32 {
        self.fill_rect_count
    }

    #[wasm_bindgen(getter = drawImageCount)]
    pub fn draw_image_count(&self) -> u32 {
        self.draw_image_count
    }

    #[wasm_bindgen(getter = registeredFontCount)]
    pub fn registered_font_count(&self) -> u32 {
        self.registered_font_count
    }

    #[wasm_bindgen(getter = registeredImageCount)]
    pub fn registered_image_count(&self) -> u32 {
        self.registered_image_count
    }

    #[wasm_bindgen(getter = semanticsNodes)]
    pub fn semantics_nodes(&self) -> Vec<JsSemanticNode> {
        self.semantics_nodes.clone()
    }

    #[wasm_bindgen]
    pub fn find(&self, query: JsValue) -> Result<Vec<JsSemanticNode>> {
        let query = <Option<JsSemanticsQuery> as FromJs>::from_js(&query)?;
        let query = match query {
            Some(query) => query.into_binding(Some(true))?,
            None => BindingSemanticQuery {
                visible: Some(true),
                ..BindingSemanticQuery::default()
            },
        };
        Ok(self
            .inner_snapshot()
            .query(&query)
            .into_iter()
            .map(Into::into)
            .collect())
    }

    #[wasm_bindgen(js_name = getOne)]
    pub fn get_one(&self, query: JsValue) -> Result<JsSemanticNode> {
        let query = <Option<JsSemanticsQuery> as FromJs>::from_js(&query)?;
        let query = match query {
            Some(query) => query.into_binding(None)?,
            None => BindingSemanticQuery::default(),
        };
        self.inner_snapshot()
            .query_one(&query)
            .map(Into::into)
            .map_err(runtime_error)
    }
}

impl JsRenderSnapshot {
    pub(crate) fn inner_snapshot(&self) -> BindingRenderSnapshot {
        BindingRenderSnapshot {
            command_count: self.command_count as usize,
            semantics_count: self.semantics_count as usize,
            semantics_nodes: self
                .semantics_nodes
                .iter()
                .map(|node| node.inner.clone())
                .collect(),
            semantics_roles: self.semantics_roles.clone(),
            semantics_names: self.semantics_names.clone(),
            semantics_values: self.semantics_values.clone(),
            semantics_descriptions: self.semantics_descriptions.clone(),
            semantics_checked: self.semantics_checked.clone(),
            semantics_busy: self.semantics_busy.clone(),
            semantics_editable_multiline: self.semantics_editable_multiline.clone(),
            semantics_disabled: self.semantics_disabled.clone(),
            semantics_focused: self.semantics_focused.clone(),
            semantics_hidden: self.semantics_hidden.clone(),
            semantics_hovered: self.semantics_hovered.clone(),
            semantics_selected: self.semantics_selected.clone(),
            semantics_expanded: self.semantics_expanded.clone(),
            fill_rect_count: self.fill_rect_count as usize,
            draw_image_count: self.draw_image_count as usize,
            registered_font_count: self.registered_font_count as usize,
            registered_image_count: self.registered_image_count as usize,
        }
    }
}

impl From<BindingRenderSnapshot> for JsRenderSnapshot {
    fn from(value: BindingRenderSnapshot) -> Self {
        Self {
            command_count: value.command_count as u32,
            semantics_count: value.semantics_count as u32,
            semantics_nodes: value.semantics_nodes.into_iter().map(Into::into).collect(),
            semantics_roles: value.semantics_roles,
            semantics_names: value.semantics_names,
            semantics_values: value.semantics_values,
            semantics_descriptions: value.semantics_descriptions,
            semantics_checked: value.semantics_checked,
            semantics_busy: value.semantics_busy,
            semantics_editable_multiline: value.semantics_editable_multiline,
            semantics_disabled: value.semantics_disabled,
            semantics_focused: value.semantics_focused,
            semantics_hidden: value.semantics_hidden,
            semantics_hovered: value.semantics_hovered,
            semantics_selected: value.semantics_selected,
            semantics_expanded: value.semantics_expanded,
            fill_rect_count: value.fill_rect_count as u32,
            draw_image_count: value.draw_image_count as u32,
            registered_font_count: value.registered_font_count as u32,
            registered_image_count: value.registered_image_count as u32,
        }
    }
}

#[wasm_bindgen(js_name = renderWidget)]
pub fn render_widget(
    widget: &JsWidget,
    event: Option<ClassArg<JsEvent>>,
) -> Result<JsRenderSnapshot> {
    let event = event.map(|value| value.value()).transpose()?;
    let event = event.as_ref();
    if let Ok(binding) = widget.binding_widget() {
        return render_binding_widget(binding, event);
    }
    render_foreign_widget(widget.into_sui_widget()?, event)
}

pub(crate) fn render_binding_widget(
    widget: BindingWidget,
    event: Option<&JsEvent>,
) -> Result<JsRenderSnapshot> {
    let app = BindingApp::new().with_window(BindingWindow::new("JavaScript widget", widget));
    if let Some(event) = event {
        let mut runtime = app.start().map_err(runtime_error)?;
        let window_id = runtime.window_id_at(0).map_err(runtime_error)?;
        let _ = runtime.render_window(window_id).map_err(runtime_error)?;
        runtime
            .handle_event(window_id, event.binding_event())
            .map_err(runtime_error)?;
        return runtime
            .render_window(window_id)
            .map(JsRenderSnapshot::from)
            .map_err(runtime_error);
    }
    app.render_window(0)
        .map(JsRenderSnapshot::from)
        .map_err(runtime_error)
}

pub(crate) fn render_foreign_widget(
    widget: ForeignWidget,
    event: Option<&JsEvent>,
) -> Result<JsRenderSnapshot> {
    let mut runtime = RuntimeApplication::new()
        .window(WindowBuilder::new().title("JavaScript widget").root(widget))
        .build()
        .map_err(runtime_error)?;
    let window_id = runtime.window_ids()[0];
    let mut output = runtime.render(window_id).map_err(runtime_error)?;
    if let Some(event) = event {
        runtime
            .handle_event(
                window_id,
                event
                    .binding_event()
                    .into_sui_event()
                    .map_err(runtime_error)?,
            )
            .map_err(runtime_error)?;
        output = runtime.render(window_id).map_err(runtime_error)?;
    }
    let mut command_count = 0;
    let mut fill_rect_count = 0;
    let mut draw_image_count = 0;
    output.frame.scene.visit_commands(&mut |command| {
        command_count += 1;
        match command {
            SceneCommand::FillRect { .. } => fill_rect_count += 1,
            SceneCommand::DrawImage { .. } | SceneCommand::DrawImageQuad { .. } => {
                draw_image_count += 1;
            }
            _ => {}
        }
    });
    Ok(JsRenderSnapshot {
        command_count,
        semantics_count: output.semantics.len() as u32,
        semantics_nodes: binding_semantics_nodes(&output.semantics)
            .into_iter()
            .map(Into::into)
            .collect(),
        semantics_roles: binding_semantics_roles(&output.semantics),
        semantics_names: binding_semantics_names(&output.semantics),
        semantics_values: binding_semantics_values(&output.semantics),
        semantics_descriptions: binding_semantics_descriptions(&output.semantics),
        semantics_checked: binding_semantics_checked(&output.semantics),
        semantics_busy: binding_semantics_busy(&output.semantics),
        semantics_editable_multiline: binding_semantics_editable_multiline(&output.semantics),
        semantics_disabled: binding_semantics_disabled(&output.semantics),
        semantics_focused: binding_semantics_focused(&output.semantics),
        semantics_hidden: binding_semantics_hidden(&output.semantics),
        semantics_hovered: binding_semantics_hovered(&output.semantics),
        semantics_selected: binding_semantics_selected(&output.semantics),
        semantics_expanded: binding_semantics_expanded(&output.semantics),
        fill_rect_count,
        draw_image_count,
        registered_font_count: output.frame.font_registry.len() as u32,
        registered_image_count: output.frame.image_registry.len() as u32,
    })
}
