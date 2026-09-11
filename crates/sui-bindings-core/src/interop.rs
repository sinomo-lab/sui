use std::fmt;
use std::sync::Arc;
use sui::ColorSpace;
use sui::Size;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NativeGraphicsBackend {
    Cpu,
    Wgpu,
    WebGpu,
    D3d12,
    Metal,
    Vulkan,
    OpenGl,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RendererInteropTier {
    CpuUpload,
    SharedTexture,
    SharedRenderTarget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RendererInteropCapabilities {
    pub backend: NativeGraphicsBackend,
    pub cpu_upload: bool,
    pub shared_texture: bool,
    pub shared_render_target: bool,
}

impl RendererInteropCapabilities {
    pub const fn cpu_only(backend: NativeGraphicsBackend) -> Self {
        Self {
            backend,
            cpu_upload: true,
            shared_texture: false,
            shared_render_target: false,
        }
    }

    pub const fn supports(self, tier: RendererInteropTier) -> bool {
        match tier {
            RendererInteropTier::CpuUpload => self.cpu_upload,
            RendererInteropTier::SharedTexture => self.shared_texture,
            RendererInteropTier::SharedRenderTarget => self.shared_render_target,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExternalTextureFormat {
    Rgba8Unorm,
    Bgra8Unorm,
    Rgba16Float,
}

impl ExternalTextureFormat {
    pub const fn bytes_per_pixel(self) -> Option<usize> {
        match self {
            Self::Rgba8Unorm | Self::Bgra8Unorm => Some(4),
            Self::Rgba16Float => Some(8),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ExternalBackendHandle {
    pub(crate) id: u64,
}

impl ExternalBackendHandle {
    pub const fn new(id: u64) -> Self {
        Self { id }
    }

    pub const fn id(self) -> u64 {
        self.id
    }

    pub const fn is_empty(self) -> bool {
        self.id == 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExternalSync {
    None,
    Generation(u64),
    TimelineValue {
        handle: ExternalBackendHandle,
        value: u64,
    },
    Fence {
        handle: ExternalBackendHandle,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExternalTextureDescriptor {
    CpuRgba8 {
        size: Size,
        pixels: Arc<[u8]>,
        generation: u64,
    },
    SharedTexture {
        backend: NativeGraphicsBackend,
        size: Size,
        format: ExternalTextureFormat,
        color_space: ColorSpace,
        handle: ExternalBackendHandle,
        sync: ExternalSync,
    },
    SharedRenderTarget {
        backend: NativeGraphicsBackend,
        size: Size,
        format: ExternalTextureFormat,
        color_space: ColorSpace,
        handle: ExternalBackendHandle,
        sync: ExternalSync,
    },
}

impl ExternalTextureDescriptor {
    pub fn cpu_rgba8(size: Size, pixels: impl Into<Arc<[u8]>>, generation: u64) -> Self {
        Self::CpuRgba8 {
            size,
            pixels: pixels.into(),
            generation,
        }
    }

    pub fn size(&self) -> Size {
        match self {
            Self::CpuRgba8 { size, .. }
            | Self::SharedTexture { size, .. }
            | Self::SharedRenderTarget { size, .. } => *size,
        }
    }

    pub const fn tier(&self) -> RendererInteropTier {
        match self {
            Self::CpuRgba8 { .. } => RendererInteropTier::CpuUpload,
            Self::SharedTexture { .. } => RendererInteropTier::SharedTexture,
            Self::SharedRenderTarget { .. } => RendererInteropTier::SharedRenderTarget,
        }
    }

    pub fn validate(&self) -> Result<(), ExternalTextureValidationError> {
        validate_external_size(self.size())?;
        match self {
            Self::CpuRgba8 { size, pixels, .. } => {
                let expected = external_pixel_len(*size, 4)?;
                if pixels.len() != expected {
                    return Err(ExternalTextureValidationError::InvalidPixelLength {
                        expected,
                        actual: pixels.len(),
                    });
                }
            }
            Self::SharedTexture {
                handle,
                sync,
                format,
                ..
            }
            | Self::SharedRenderTarget {
                handle,
                sync,
                format,
                ..
            } => {
                if handle.is_empty() {
                    return Err(ExternalTextureValidationError::EmptyHandle);
                }
                if let ExternalSync::TimelineValue { handle, .. } | ExternalSync::Fence { handle } =
                    sync
                    && handle.is_empty()
                {
                    return Err(ExternalTextureValidationError::EmptySyncHandle);
                }
                if format.bytes_per_pixel().is_none() {
                    return Err(ExternalTextureValidationError::UnsupportedFormat);
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExternalTextureValidationError {
    NonFiniteSize,
    NonPositiveSize,
    NonIntegerSize,
    SizeOverflow,
    InvalidPixelLength { expected: usize, actual: usize },
    EmptyHandle,
    EmptySyncHandle,
    UnsupportedFormat,
}

impl fmt::Display for ExternalTextureValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonFiniteSize => f.write_str("external texture size must be finite"),
            Self::NonPositiveSize => f.write_str("external texture size must be positive"),
            Self::NonIntegerSize => f.write_str("external texture size must use whole pixels"),
            Self::SizeOverflow => f.write_str("external texture byte length overflowed"),
            Self::InvalidPixelLength { expected, actual } => write!(
                f,
                "external CPU texture has {actual} bytes, expected {expected}"
            ),
            Self::EmptyHandle => f.write_str("external texture handle must be non-empty"),
            Self::EmptySyncHandle => f.write_str("external sync handle must be non-empty"),
            Self::UnsupportedFormat => f.write_str("external texture format is unsupported"),
        }
    }
}

impl std::error::Error for ExternalTextureValidationError {}

pub(crate) fn validate_external_size(size: Size) -> Result<(), ExternalTextureValidationError> {
    if !size.width.is_finite() || !size.height.is_finite() {
        return Err(ExternalTextureValidationError::NonFiniteSize);
    }
    if size.width <= 0.0 || size.height <= 0.0 {
        return Err(ExternalTextureValidationError::NonPositiveSize);
    }
    if size.width.fract() != 0.0 || size.height.fract() != 0.0 {
        return Err(ExternalTextureValidationError::NonIntegerSize);
    }
    if size.width > u32::MAX as f32 || size.height > u32::MAX as f32 {
        return Err(ExternalTextureValidationError::SizeOverflow);
    }
    Ok(())
}

pub(crate) fn external_pixel_len(
    size: Size,
    bytes_per_pixel: usize,
) -> Result<usize, ExternalTextureValidationError> {
    validate_external_size(size)?;
    let width = size.width as usize;
    let height = size.height as usize;
    width
        .checked_mul(height)
        .and_then(|pixels| pixels.checked_mul(bytes_per_pixel))
        .ok_or(ExternalTextureValidationError::SizeOverflow)
}
