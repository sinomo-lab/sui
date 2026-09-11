use crate::errors::{
    BINDING_APP_FONT_HANDLE_NAMESPACE, BINDING_APP_FONT_SLOT_MASK,
    BINDING_APP_IMAGE_HANDLE_NAMESPACE, BINDING_APP_IMAGE_SLOT_MASK,
    BINDING_LOCAL_IMAGE_HANDLE_NAMESPACE, BINDING_LOCAL_IMAGE_SLOT_MASK,
};
use crate::paint::PaintCommand;
use sui::FontHandle;
use sui::ImageHandle;
use sui::WindowId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BindingWindowId(pub(crate) WindowId);

impl BindingWindowId {
    pub const fn new(raw: u64) -> Self {
        Self(WindowId::new(raw))
    }

    pub const fn get(self) -> u64 {
        self.0.get()
    }

    pub(crate) fn into_sui(self) -> WindowId {
        self.0
    }
}

impl From<WindowId> for BindingWindowId {
    fn from(value: WindowId) -> Self {
        Self(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BindingFontHandle(pub(crate) FontHandle);

impl BindingFontHandle {
    pub const fn new(raw: u64) -> Self {
        Self(FontHandle::new(raw))
    }

    pub const fn app_resource(slot: u64) -> Self {
        Self(FontHandle::new(
            BINDING_APP_FONT_HANDLE_NAMESPACE | (slot & BINDING_APP_FONT_SLOT_MASK),
        ))
    }

    pub const fn get(self) -> u64 {
        self.0.get()
    }

    pub const fn into_sui(self) -> FontHandle {
        self.0
    }
}

impl From<FontHandle> for BindingFontHandle {
    fn from(value: FontHandle) -> Self {
        Self(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BindingImageHandle(pub(crate) ImageHandle);

impl BindingImageHandle {
    pub const fn new(raw: u64) -> Self {
        Self(ImageHandle::new(raw))
    }

    pub const fn local(slot: u64) -> Self {
        Self(ImageHandle::new(
            BINDING_LOCAL_IMAGE_HANDLE_NAMESPACE | (slot & BINDING_LOCAL_IMAGE_SLOT_MASK),
        ))
    }

    pub const fn app_resource(slot: u64) -> Self {
        Self(ImageHandle::new(
            BINDING_APP_IMAGE_HANDLE_NAMESPACE | (slot & BINDING_APP_IMAGE_SLOT_MASK),
        ))
    }

    pub const fn get(self) -> u64 {
        self.0.get()
    }

    pub const fn local_slot(self) -> Option<u64> {
        binding_local_image_slot(self.0)
    }

    pub const fn into_sui(self) -> ImageHandle {
        self.0
    }
}

impl From<ImageHandle> for BindingImageHandle {
    fn from(value: ImageHandle) -> Self {
        Self(value)
    }
}

pub(crate) const fn binding_local_image_slot(handle: ImageHandle) -> Option<u64> {
    let raw = handle.get();
    if raw & BINDING_LOCAL_IMAGE_HANDLE_NAMESPACE == BINDING_LOCAL_IMAGE_HANDLE_NAMESPACE {
        Some(raw & BINDING_LOCAL_IMAGE_SLOT_MASK)
    } else {
        None
    }
}

pub fn resolve_binding_image_slots(
    commands: &mut [PaintCommand],
    mut resolve: impl FnMut(u64) -> ImageHandle,
) {
    for command in commands {
        match command {
            PaintCommand::DrawImage { source, .. } => {
                if let Some(slot) = binding_local_image_slot(source.image) {
                    source.image = resolve(slot);
                }
            }
            PaintCommand::DrawImageQuad { source, .. } => {
                if let Some(slot) = binding_local_image_slot(source.image) {
                    source.image = resolve(slot);
                }
            }
            _ => {}
        }
    }
}
