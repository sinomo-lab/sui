use crate::prelude::*;

#[wasm_bindgen(js_name = Point)]
#[derive(Debug, Clone, Copy)]
pub struct JsPoint {
    pub x: f64,
    pub y: f64,
}

js_class!(JsPoint => "Point");

#[wasm_bindgen(js_class = Point)]
impl JsPoint {
    #[wasm_bindgen(constructor)]
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

impl From<sui_crate::Point> for JsPoint {
    fn from(value: sui_crate::Point) -> Self {
        Self::new(f64::from(value.x), f64::from(value.y))
    }
}

impl From<JsPoint> for sui_crate::Point {
    fn from(value: JsPoint) -> Self {
        Self::new(value.x as f32, value.y as f32)
    }
}

impl From<JsPoint> for Vector {
    fn from(value: JsPoint) -> Self {
        Self::new(value.x as f32, value.y as f32)
    }
}

#[wasm_bindgen(js_name = Modifiers)]
#[derive(Debug, Clone, Copy, Default)]
pub struct JsModifiers {
    pub shift: bool,
    pub control: bool,
    pub alt: bool,
    pub meta: bool,
}

js_class!(JsModifiers => "Modifiers");

#[wasm_bindgen(js_class = Modifiers)]
impl JsModifiers {
    #[wasm_bindgen(constructor)]
    pub fn new(
        shift: Option<bool>,
        control: Option<bool>,
        alt: Option<bool>,
        meta: Option<bool>,
    ) -> Self {
        Self {
            shift: shift.unwrap_or(false),
            control: control.unwrap_or(false),
            alt: alt.unwrap_or(false),
            meta: meta.unwrap_or(false),
        }
    }
}

impl From<BindingModifiers> for JsModifiers {
    fn from(value: BindingModifiers) -> Self {
        Self {
            shift: value.shift,
            control: value.control,
            alt: value.alt,
            meta: value.meta,
        }
    }
}

impl From<JsModifiers> for BindingModifiers {
    fn from(value: JsModifiers) -> Self {
        Self {
            shift: value.shift,
            control: value.control,
            alt: value.alt,
            meta: value.meta,
        }
    }
}

#[wasm_bindgen(js_name = Size)]
#[derive(Debug, Clone, Copy)]
pub struct JsSize {
    pub width: f64,
    pub height: f64,
}

js_class!(JsSize => "Size");

#[wasm_bindgen(js_class = Size)]
impl JsSize {
    #[wasm_bindgen(constructor)]
    pub fn new(width: f64, height: f64) -> Self {
        Self { width, height }
    }
}

impl JsSize {
    pub(crate) fn to_sui(&self) -> Size {
        Size::new(self.width as f32, self.height as f32)
    }
}

impl From<Size> for JsSize {
    fn from(value: Size) -> Self {
        Self::new(f64::from(value.width), f64::from(value.height))
    }
}

impl From<JsSize> for Size {
    fn from(value: JsSize) -> Self {
        Self::new(value.width as f32, value.height as f32)
    }
}

#[wasm_bindgen(js_name = Rect)]
#[derive(Debug, Clone, Copy)]
pub struct JsRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

js_class!(JsRect => "Rect");

#[wasm_bindgen(js_class = Rect)]
impl JsRect {
    #[wasm_bindgen(constructor)]
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    #[wasm_bindgen(getter = origin)]
    pub fn origin(&self) -> JsPoint {
        JsPoint::new(self.x, self.y)
    }

    #[wasm_bindgen(getter = size)]
    pub fn size(&self) -> JsSize {
        JsSize::new(self.width, self.height)
    }
}

impl From<Rect> for JsRect {
    fn from(value: Rect) -> Self {
        Self::new(
            f64::from(value.x()),
            f64::from(value.y()),
            f64::from(value.width()),
            f64::from(value.height()),
        )
    }
}

impl From<JsRect> for Rect {
    fn from(value: JsRect) -> Self {
        Self::new(
            value.x as f32,
            value.y as f32,
            value.width as f32,
            value.height as f32,
        )
    }
}

#[wasm_bindgen(js_name = Path)]
#[derive(Debug, Clone)]
pub struct JsPath {
    pub(crate) inner: Path,
}

js_class!(JsPath => "Path");

#[wasm_bindgen(js_class = Path)]
impl JsPath {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self { inner: Path::new() }
    }

    #[wasm_bindgen]
    pub fn rect(rect: &JsRect) -> Self {
        Self {
            inner: Path::rect((*rect).into()),
        }
    }

    #[wasm_bindgen]
    pub fn circle(center: &JsPoint, radius: f64) -> Self {
        Self {
            inner: Path::circle((*center).into(), radius as f32),
        }
    }

    #[wasm_bindgen(js_name = roundedRect)]
    pub fn rounded_rect(rect: &JsRect, radius: f64) -> Self {
        Self {
            inner: Path::rounded_rect((*rect).into(), radius as f32),
        }
    }

    #[wasm_bindgen]
    pub fn arc(center: &JsPoint, radius: f64, start_angle: f64, sweep_angle: f64) -> Self {
        Self {
            inner: Path::arc(
                (*center).into(),
                radius as f32,
                start_angle as f32,
                sweep_angle as f32,
            ),
        }
    }

    #[wasm_bindgen(getter = bounds)]
    pub fn bounds(&self) -> JsRect {
        self.inner.bounds().into()
    }

    #[wasm_bindgen(getter = elementCount)]
    pub fn element_count(&self) -> u32 {
        self.inner.elements().len() as u32
    }

    #[wasm_bindgen(js_name = isEmpty)]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

impl Default for JsPath {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen(js_name = PathBuilder)]
#[derive(Debug, Clone, Default)]
pub struct JsPathBuilder {
    pub(crate) inner: PathBuilder,
}

js_class!(JsPathBuilder => "PathBuilder");

#[wasm_bindgen(js_class = PathBuilder)]
impl JsPathBuilder {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            inner: PathBuilder::new(),
        }
    }

    #[wasm_bindgen(js_name = moveTo)]
    pub fn move_to(&mut self, point: &JsPoint) {
        self.inner.move_to((*point).into());
    }

    #[wasm_bindgen(js_name = lineTo)]
    pub fn line_to(&mut self, point: &JsPoint) {
        self.inner.line_to((*point).into());
    }

    #[wasm_bindgen(js_name = quadTo)]
    pub fn quad_to(&mut self, ctrl: &JsPoint, to: &JsPoint) {
        self.inner.quad_to((*ctrl).into(), (*to).into());
    }

    #[wasm_bindgen(js_name = cubicTo)]
    pub fn cubic_to(&mut self, ctrl1: &JsPoint, ctrl2: &JsPoint, to: &JsPoint) {
        self.inner
            .cubic_to((*ctrl1).into(), (*ctrl2).into(), (*to).into());
    }

    #[wasm_bindgen]
    pub fn close(&mut self) {
        self.inner.close();
    }

    #[wasm_bindgen(js_name = pushRect)]
    pub fn push_rect(&mut self, rect: &JsRect) {
        self.inner.push_rect((*rect).into());
    }

    #[wasm_bindgen(js_name = pushCircle)]
    pub fn push_circle(&mut self, center: &JsPoint, radius: f64) {
        self.inner.push_circle((*center).into(), radius as f32);
    }

    #[wasm_bindgen(js_name = pushRoundedRect)]
    pub fn push_rounded_rect(&mut self, rect: &JsRect, radius: f64) {
        self.inner.push_rounded_rect((*rect).into(), radius as f32);
    }

    #[wasm_bindgen(js_name = pushArc)]
    pub fn push_arc(&mut self, center: &JsPoint, radius: f64, start_angle: f64, sweep_angle: f64) {
        self.inner.push_arc(
            (*center).into(),
            radius as f32,
            start_angle as f32,
            sweep_angle as f32,
        );
    }

    #[wasm_bindgen]
    pub fn build(&self) -> JsPath {
        JsPath {
            inner: self.inner.clone().build(),
        }
    }
}

#[wasm_bindgen(js_name = Transform)]
#[derive(Debug, Clone, Copy)]
pub struct JsTransform {
    pub xx: f64,
    pub yx: f64,
    pub xy: f64,
    pub yy: f64,
    pub dx: f64,
    pub dy: f64,
}

js_class!(JsTransform => "Transform");

#[wasm_bindgen(js_class = Transform)]
impl JsTransform {
    #[wasm_bindgen(constructor)]
    pub fn new(xx: f64, yx: f64, xy: f64, yy: f64, dx: f64, dy: f64) -> Self {
        Self {
            xx,
            yx,
            xy,
            yy,
            dx,
            dy,
        }
    }

    #[wasm_bindgen]
    pub fn identity() -> Self {
        Self::from(Transform::IDENTITY)
    }

    #[wasm_bindgen]
    pub fn translation(x: f64, y: f64) -> Self {
        Self::from(Transform::translation(x as f32, y as f32))
    }

    #[wasm_bindgen]
    pub fn scale(x: f64, y: f64) -> Self {
        Self::from(Transform::scale(x as f32, y as f32))
    }

    #[wasm_bindgen]
    pub fn rotation(radians: f64) -> Self {
        Self::from(Transform::rotation(radians as f32))
    }

    #[wasm_bindgen]
    pub fn then(&self, next: &JsTransform) -> Self {
        Transform::from(*self).then((*next).into()).into()
    }
}

impl From<Transform> for JsTransform {
    fn from(value: Transform) -> Self {
        Self::new(
            f64::from(value.xx),
            f64::from(value.yx),
            f64::from(value.xy),
            f64::from(value.yy),
            f64::from(value.dx),
            f64::from(value.dy),
        )
    }
}

impl From<JsTransform> for Transform {
    fn from(value: JsTransform) -> Self {
        Self::new(
            value.xx as f32,
            value.yx as f32,
            value.xy as f32,
            value.yy as f32,
            value.dx as f32,
            value.dy as f32,
        )
    }
}

#[wasm_bindgen(js_name = Color)]
#[derive(Debug, Clone, Copy)]
pub struct JsColor {
    pub red: f64,
    pub green: f64,
    pub blue: f64,
    pub alpha: f64,
}

js_class!(JsColor => "Color");

#[wasm_bindgen(js_class = Color)]
impl JsColor {
    #[wasm_bindgen(constructor)]
    pub fn new(red: f64, green: f64, blue: f64, alpha: Option<f64>) -> Self {
        Self {
            red,
            green,
            blue,
            alpha: alpha.unwrap_or(1.0),
        }
    }
}

impl From<JsColor> for Color {
    fn from(value: JsColor) -> Self {
        Self::rgba(
            value.red as f32,
            value.green as f32,
            value.blue as f32,
            value.alpha as f32,
        )
    }
}

impl From<Color> for JsColor {
    fn from(value: Color) -> Self {
        Self::new(
            f64::from(value.red),
            f64::from(value.green),
            f64::from(value.blue),
            Some(f64::from(value.alpha)),
        )
    }
}

#[wasm_bindgen(js_name = Shadow)]
#[derive(Debug, Clone, Copy)]
pub struct JsShadow {
    pub(crate) offset_x: f64,
    pub(crate) offset_y: f64,
    pub(crate) blur: f64,
    pub(crate) spread: f64,
    pub(crate) color: JsColor,
    pub(crate) placement: sui_crate::ShadowPlacement,
}

js_class!(JsShadow => "Shadow");

#[wasm_bindgen(js_class = Shadow)]
impl JsShadow {
    /// `placement` is `"behind"` (the default), `"outside"` for a glow that
    /// leaves the box clear, or `"inside"` for an inset shadow.
    #[wasm_bindgen(constructor)]
    pub fn new(
        offset_x: f64,
        offset_y: f64,
        blur: f64,
        spread: f64,
        color: &JsColor,
        placement: Option<String>,
    ) -> Result<Self> {
        let placement = match placement {
            Some(name) => binding_shadow_placement_from_name(&name)
                .ok_or_else(|| invalid_arg(format!("unknown shadow placement '{name}'")))?,
            None => sui_crate::ShadowPlacement::Behind,
        };
        Ok(Self {
            offset_x,
            offset_y,
            blur,
            spread,
            color: *color,
            placement,
        })
    }

    #[wasm_bindgen(getter = placement)]
    pub fn placement(&self) -> String {
        binding_shadow_placement_name(self.placement).to_owned()
    }

    #[wasm_bindgen(getter = offsetX)]
    pub fn offset_x(&self) -> f64 {
        self.offset_x
    }

    #[wasm_bindgen(getter = offsetY)]
    pub fn offset_y(&self) -> f64 {
        self.offset_y
    }

    #[wasm_bindgen(getter = blur)]
    pub fn blur(&self) -> f64 {
        self.blur
    }

    #[wasm_bindgen(getter = spread)]
    pub fn spread(&self) -> f64 {
        self.spread
    }

    #[wasm_bindgen(getter = color)]
    pub fn color(&self) -> JsColor {
        self.color
    }
}

impl From<JsShadow> for ShadowParams {
    fn from(value: JsShadow) -> Self {
        Self::new(
            value.offset_x as f32,
            value.offset_y as f32,
            value.blur as f32,
            value.spread as f32,
            value.color.into(),
        )
        .with_placement(value.placement)
    }
}

#[wasm_bindgen(js_name = Constraints)]
#[derive(Debug, Clone, Copy)]
pub struct JsConstraints {
    pub(crate) min: JsSize,
    pub(crate) max: JsSize,
}

js_class!(JsConstraints => "Constraints");

#[wasm_bindgen(js_class = Constraints)]
impl JsConstraints {
    #[wasm_bindgen(constructor)]
    pub fn new(min: &JsSize, max: &JsSize) -> Self {
        Self {
            min: *min,
            max: *max,
        }
    }

    #[wasm_bindgen(getter = min)]
    pub fn min(&self) -> JsSize {
        self.min
    }

    #[wasm_bindgen(getter = max)]
    pub fn max(&self) -> JsSize {
        self.max
    }

    #[wasm_bindgen]
    pub fn clamp(&self, size: &JsSize) -> JsSize {
        self.to_sui().clamp((*size).into()).into()
    }

    #[wasm_bindgen]
    pub fn loosen(&self) -> Self {
        Self::from(self.to_sui().loosen())
    }
}

impl JsConstraints {
    pub(crate) fn to_sui(&self) -> Constraints {
        Constraints::new(self.min.into(), self.max.into())
    }
}

impl From<Constraints> for JsConstraints {
    fn from(value: Constraints) -> Self {
        Self {
            min: value.min.into(),
            max: value.max.into(),
        }
    }
}
