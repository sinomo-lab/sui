use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use std::sync::Condvar;
use std::sync::Mutex;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::time::Duration;
use sui_core::Error;
use sui_core::ImageHandle;
use sui_core::Result;
use sui_scene::RegisteredExternalImage;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RendererInterop {
    pub raw_wgpu_enabled: bool,
}

/// Cloneable access to the renderer-owned WGPU device and queue.
///
/// The handles refer to the exact device used by SUI. Applications can use
/// them to populate textures or run conversion passes, then register the
/// resulting sampled texture through [`WgpuExternalTextureRegistry`].
#[derive(Debug, Clone)]
pub struct WgpuExternalTextureContext {
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    pub(crate) adapter_info: wgpu::AdapterInfo,
}

impl WgpuExternalTextureContext {
    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    /// Identifies the physical/logical adapter backing the shared device.
    ///
    /// External renderers can persist the backend, adapter, and driver fields
    /// alongside captures produced through SUI's device.
    pub fn adapter_info(&self) -> &wgpu::AdapterInfo {
        &self.adapter_info
    }
}

#[derive(Debug, Clone)]
pub(crate) struct WgpuExternalTextureEntry {
    pub(crate) texture: wgpu::Texture,
    pub(crate) view: wgpu::TextureView,
    pub(crate) descriptor: RegisteredExternalImage,
    pub(crate) binding_revision: u64,
    pub(crate) content_revision: Option<u64>,
}

#[derive(Default)]
pub(crate) struct WgpuExternalTextureRegistryInner {
    pub(crate) context: Mutex<Option<WgpuExternalTextureContext>>,
    pub(crate) context_ready: Condvar,
    pub(crate) textures: Mutex<HashMap<ImageHandle, WgpuExternalTextureEntry>>,
    pub(crate) next_binding_revision: AtomicU64,
}

/// Shared registry for app-owned textures sampled by SUI's normal image draw
/// path.
///
/// This contract is intentionally media-agnostic. The application owns the
/// producer, update cadence, pixel conversion, and texture contents. SUI owns
/// only the renderer device handoff and sampled-texture lifetime needed to
/// compose the resource with ordinary scene transforms and clips.
#[derive(Clone, Default)]
pub struct WgpuExternalTextureRegistry {
    pub(crate) inner: Arc<WgpuExternalTextureRegistryInner>,
}

impl fmt::Debug for WgpuExternalTextureRegistry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WgpuExternalTextureRegistry")
            .field("attached", &self.context().is_some())
            .field("len", &self.len())
            .finish()
    }
}

impl WgpuExternalTextureRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the renderer device once the first render target initializes.
    pub fn context(&self) -> Option<WgpuExternalTextureContext> {
        self.inner
            .context
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    /// Wait for renderer initialization without tying the registry to a
    /// platform event-loop type.
    pub fn wait_for_context(&self, timeout: Duration) -> Option<WgpuExternalTextureContext> {
        let context = self
            .inner
            .context
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let (context, _) = self
            .inner
            .context_ready
            .wait_timeout_while(context, timeout, |context| context.is_none())
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        context.clone()
    }

    /// Upload tightly packed sRGB RGBA8 pixels into a persistent texture.
    /// Reusing `handle` and dimensions updates the existing allocation.
    pub fn upload_rgba8(
        &self,
        handle: ImageHandle,
        width: u32,
        height: u32,
        pixels: Arc<[u8]>,
        revision: u64,
    ) -> Result<RegisteredExternalImage> {
        let context = self.context().ok_or_else(|| {
            Error::new("external texture registry is not attached to an initialized renderer")
        })?;
        let descriptor = RegisteredExternalImage::new(width, height)?;
        let row_bytes = width
            .checked_mul(4)
            .ok_or_else(|| Error::new("external RGBA8 image row size overflow"))?;
        let expected_len = row_bytes
            .checked_mul(height)
            .map(|len| len as usize)
            .ok_or_else(|| Error::new("external RGBA8 image byte size overflow"))?;
        if pixels.len() != expected_len {
            return Err(Error::new(format!(
                "external RGBA8 image data length {} does not match expected size {} for a {}x{} image",
                pixels.len(),
                expected_len,
                width,
                height
            )));
        }

        let mut textures = self
            .inner
            .textures
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let reusable = textures.get_mut(&handle).filter(|entry| {
            entry.descriptor == descriptor
                && entry.texture.format() == wgpu::TextureFormat::Rgba8UnormSrgb
                && entry
                    .texture
                    .usage()
                    .contains(wgpu::TextureUsages::COPY_DST)
        });
        if let Some(entry) = reusable {
            if entry.content_revision == Some(revision) {
                return Ok(entry.descriptor);
            }
            Self::write_rgba8(&context.queue, &entry.texture, width, height, &pixels);
            entry.content_revision = Some(revision);
            return Ok(entry.descriptor);
        }

        let texture = context.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("SUI external RGBA8 texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        Self::write_rgba8(&context.queue, &texture, width, height, &pixels);
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let binding_revision = self.next_binding_revision();
        textures.insert(
            handle,
            WgpuExternalTextureEntry {
                texture,
                view,
                descriptor,
                binding_revision,
                content_revision: Some(revision),
            },
        );
        Ok(descriptor)
    }

    /// Register a sampled texture created from this registry's renderer
    /// context. This supports app-owned GPU conversion/render passes without
    /// exposing backend-native shared-handle policy in SUI.
    pub fn register_texture(
        &self,
        handle: ImageHandle,
        texture: wgpu::Texture,
    ) -> Result<RegisteredExternalImage> {
        let context = self.context().ok_or_else(|| {
            Error::new("external texture registry is not attached to an initialized renderer")
        })?;
        if texture.dimension() != wgpu::TextureDimension::D2
            || texture.depth_or_array_layers() != 1
            || texture.sample_count() != 1
        {
            return Err(Error::new(
                "external texture must be a single-sampled two-dimensional texture",
            ));
        }
        if !texture
            .usage()
            .contains(wgpu::TextureUsages::TEXTURE_BINDING)
        {
            return Err(Error::new(
                "external texture must include TEXTURE_BINDING usage",
            ));
        }
        if texture.format().sample_type(
            Some(wgpu::TextureAspect::All),
            Some(context.device.features()),
        ) != Some(wgpu::TextureSampleType::Float { filterable: true })
        {
            return Err(Error::new(
                "external texture format must support filterable float sampling",
            ));
        }

        let descriptor = RegisteredExternalImage::new(texture.width(), texture.height())?;
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let entry = WgpuExternalTextureEntry {
            texture,
            view,
            descriptor,
            binding_revision: self.next_binding_revision(),
            content_revision: None,
        };
        self.inner
            .textures
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(handle, entry);
        Ok(descriptor)
    }

    pub fn unregister(&self, handle: ImageHandle) -> bool {
        self.inner
            .textures
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(&handle)
            .is_some()
    }

    pub fn contains(&self, handle: ImageHandle) -> bool {
        self.inner
            .textures
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .contains_key(&handle)
    }

    pub fn len(&self) -> usize {
        self.inner
            .textures
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub(crate) fn attach(
        &self,
        device: wgpu::Device,
        queue: wgpu::Queue,
        adapter_info: wgpu::AdapterInfo,
    ) {
        let mut context = self
            .inner
            .context
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if context.is_none() {
            *context = Some(WgpuExternalTextureContext {
                device,
                queue,
                adapter_info,
            });
            self.inner.context_ready.notify_all();
        }
    }

    pub(crate) fn resolve(&self, handle: ImageHandle) -> Option<WgpuExternalTextureEntry> {
        self.inner
            .textures
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(&handle)
            .cloned()
    }

    pub(crate) fn next_binding_revision(&self) -> u64 {
        self.inner
            .next_binding_revision
            .fetch_add(1, Ordering::Relaxed)
            .wrapping_add(1)
    }

    pub(crate) fn write_rgba8(
        queue: &wgpu::Queue,
        texture: &wgpu::Texture,
        width: u32,
        height: u32,
        pixels: &[u8],
    ) {
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
    }
}
