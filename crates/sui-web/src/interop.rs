use crate::prelude::*;

#[wasm_bindgen(js_name = RendererInteropCapabilities)]
#[derive(Debug, Clone, Copy)]
pub struct JsRendererInteropCapabilities {
    pub(crate) inner: RendererInteropCapabilities,
}

js_class!(JsRendererInteropCapabilities => "RendererInteropCapabilities");

#[wasm_bindgen(js_class = RendererInteropCapabilities)]
impl JsRendererInteropCapabilities {
    #[wasm_bindgen(constructor)]
    pub fn new(
        backend: String,
        cpu_upload: Option<bool>,
        shared_texture: Option<bool>,
        shared_render_target: Option<bool>,
    ) -> Result<Self> {
        Ok(Self {
            inner: RendererInteropCapabilities {
                backend: native_backend_from_js(&backend)?,
                cpu_upload: cpu_upload.unwrap_or(true),
                shared_texture: shared_texture.unwrap_or(false),
                shared_render_target: shared_render_target.unwrap_or(false),
            },
        })
    }

    #[wasm_bindgen(js_name = cpuOnly)]
    pub fn cpu_only(backend: String) -> Result<Self> {
        Ok(Self {
            inner: RendererInteropCapabilities::cpu_only(native_backend_from_js(&backend)?),
        })
    }

    #[wasm_bindgen]
    pub fn supports(&self, tier: String) -> Result<bool> {
        Ok(self.inner.supports(interop_tier_from_js(&tier)?))
    }

    #[wasm_bindgen(getter = backend)]
    pub fn backend(&self) -> String {
        native_backend_name(self.inner.backend).to_owned()
    }

    #[wasm_bindgen(getter = cpuUpload)]
    pub fn cpu_upload(&self) -> bool {
        self.inner.cpu_upload
    }

    #[wasm_bindgen(getter = sharedTexture)]
    pub fn shared_texture(&self) -> bool {
        self.inner.shared_texture
    }

    #[wasm_bindgen(getter = sharedRenderTarget)]
    pub fn shared_render_target(&self) -> bool {
        self.inner.shared_render_target
    }
}

#[wasm_bindgen(js_name = ExternalBackendHandle)]
#[derive(Debug, Clone, Copy)]
pub struct JsExternalBackendHandle {
    pub(crate) inner: ExternalBackendHandle,
}

js_class!(JsExternalBackendHandle => "ExternalBackendHandle");

#[wasm_bindgen(js_class = ExternalBackendHandle)]
impl JsExternalBackendHandle {
    #[wasm_bindgen(constructor)]
    pub fn new(id: String) -> Result<Self> {
        Ok(Self {
            inner: ExternalBackendHandle::new(parse_u64_string(&id, "external backend handle id")?),
        })
    }

    #[wasm_bindgen(getter = id)]
    pub fn id(&self) -> String {
        self.inner.id().to_string()
    }

    #[wasm_bindgen(getter = isEmpty)]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

#[wasm_bindgen(js_name = ExternalSync)]
#[derive(Debug, Clone, Copy)]
pub struct JsExternalSync {
    pub(crate) inner: ExternalSync,
}

js_class!(JsExternalSync => "ExternalSync");

#[wasm_bindgen(js_class = ExternalSync)]
impl JsExternalSync {
    #[wasm_bindgen]
    pub fn none() -> Self {
        Self {
            inner: ExternalSync::None,
        }
    }

    #[wasm_bindgen]
    pub fn generation(generation: String) -> Result<Self> {
        Ok(Self {
            inner: ExternalSync::Generation(parse_u64_string(
                &generation,
                "external sync generation",
            )?),
        })
    }

    #[wasm_bindgen(js_name = timelineValue)]
    pub fn timeline_value(handle: &JsExternalBackendHandle, value: String) -> Result<Self> {
        Ok(Self {
            inner: ExternalSync::TimelineValue {
                handle: handle.inner,
                value: parse_u64_string(&value, "external sync timeline value")?,
            },
        })
    }

    #[wasm_bindgen]
    pub fn fence(handle: &JsExternalBackendHandle) -> Self {
        Self {
            inner: ExternalSync::Fence {
                handle: handle.inner,
            },
        }
    }

    #[wasm_bindgen(getter = kind)]
    pub fn kind(&self) -> String {
        match self.inner {
            ExternalSync::None => "none",
            ExternalSync::Generation(_) => "generation",
            ExternalSync::TimelineValue { .. } => "timelineValue",
            ExternalSync::Fence { .. } => "fence",
        }
        .to_owned()
    }

    #[wasm_bindgen(getter = value)]
    pub fn value(&self) -> Option<String> {
        match self.inner {
            ExternalSync::Generation(value) | ExternalSync::TimelineValue { value, .. } => {
                Some(value.to_string())
            }
            ExternalSync::None | ExternalSync::Fence { .. } => None,
        }
    }
}

#[wasm_bindgen(js_name = ExternalTextureDescriptor)]
#[derive(Debug, Clone)]
pub struct JsExternalTextureDescriptor {
    pub(crate) inner: ExternalTextureDescriptor,
}

js_class!(JsExternalTextureDescriptor => "ExternalTextureDescriptor");

#[wasm_bindgen(js_class = ExternalTextureDescriptor)]
impl JsExternalTextureDescriptor {
    #[wasm_bindgen(js_name = cpuRgba8)]
    pub fn cpu_rgba8(
        size: &JsSize,
        pixels: Uint8Array,
        generation: Option<String>,
    ) -> Result<Self> {
        Ok(Self {
            inner: ExternalTextureDescriptor::cpu_rgba8(
                (*size).into(),
                pixels.to_vec(),
                generation
                    .as_deref()
                    .map(|value| parse_u64_string(value, "external texture generation"))
                    .transpose()?
                    .unwrap_or(0),
            ),
        })
    }

    #[wasm_bindgen(js_name = sharedTexture)]
    pub fn shared_texture(
        backend: String,
        size: &JsSize,
        format: String,
        handle: &JsExternalBackendHandle,
        sync: &JsExternalSync,
        color_space: Option<String>,
    ) -> Result<Self> {
        Ok(Self {
            inner: ExternalTextureDescriptor::SharedTexture {
                backend: native_backend_from_js(&backend)?,
                size: (*size).into(),
                format: external_texture_format_from_js(&format)?,
                color_space: color_space_from_js(color_space)?,
                handle: handle.inner,
                sync: sync.inner,
            },
        })
    }

    #[wasm_bindgen(js_name = sharedRenderTarget)]
    pub fn shared_render_target(
        backend: String,
        size: &JsSize,
        format: String,
        handle: &JsExternalBackendHandle,
        sync: &JsExternalSync,
        color_space: Option<String>,
    ) -> Result<Self> {
        Ok(Self {
            inner: ExternalTextureDescriptor::SharedRenderTarget {
                backend: native_backend_from_js(&backend)?,
                size: (*size).into(),
                format: external_texture_format_from_js(&format)?,
                color_space: color_space_from_js(color_space)?,
                handle: handle.inner,
                sync: sync.inner,
            },
        })
    }

    #[wasm_bindgen]
    pub fn validate(&self) -> Result<()> {
        self.inner.validate().map_err(external_texture_error)
    }

    #[wasm_bindgen(getter = tier)]
    pub fn tier(&self) -> String {
        interop_tier_name(self.inner.tier()).to_owned()
    }

    #[wasm_bindgen(getter = size)]
    pub fn size(&self) -> JsSize {
        self.inner.size().into()
    }
}
