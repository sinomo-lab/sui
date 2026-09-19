//! Reusable destination buffers and staging storage for frame uploads.
use bytemuck::Pod;

#[derive(Default)]
pub(crate) struct GpuUploads {
    belt: Option<wgpu::util::StagingBelt>,
    encoder: Option<wgpu::CommandEncoder>,
    pub(crate) bytes_written: u64,
}

impl GpuUploads {
    pub(crate) fn write_buffer(
        &mut self,
        device: &wgpu::Device,
        target: &wgpu::Buffer,
        offset: u64,
        bytes: &[u8],
    ) {
        let Some(size) = wgpu::BufferSize::new(bytes.len() as u64) else {
            return;
        };
        self.bytes_written = self.bytes_written.wrapping_add(bytes.len() as u64);
        let belt = self
            .belt
            .get_or_insert_with(|| wgpu::util::StagingBelt::new(device.clone(), 256 * 1024));
        let encoder = self.encoder.get_or_insert_with(|| {
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("SUI frame uploads"),
            })
        });
        belt.write_buffer(encoder, target, offset, size)
            .copy_from_slice(bytes);
    }

    pub(crate) fn finish(&mut self) -> Option<wgpu::CommandBuffer> {
        let encoder = self.encoder.take()?;
        // Keep copies separate from render-pass encoding: a recoverable render
        // error must not discard uploads already recorded for cached resources.
        // The caller submits these copies immediately, before the draw commands.
        self.belt
            .as_mut()
            .expect("upload belt initialized")
            .finish_and_recall_on_submit(&encoder);
        Some(encoder.finish())
    }
}

// Bound CPU copies independently of GPU buffer capacity. Oversized streams
// keep the full-upload fallback, and shadows follow their window/fragment lifetime.
const MAX_VERTEX_SHADOW_BYTES: usize = 4 * 1024 * 1024;

#[derive(Default)]
pub(crate) struct VertexBuffer {
    pub(crate) buffer: Option<wgpu::Buffer>,
    shadow: Vec<u8>,
}

impl VertexBuffer {
    pub(crate) fn upload<T: Pod>(
        &mut self,
        device: &wgpu::Device,
        uploads: &mut GpuUploads,
        label: &str,
        vertices: &[T],
    ) -> Option<wgpu::Buffer> {
        let bytes = bytemuck::cast_slice(vertices);
        if bytes.is_empty() {
            return None;
        }
        let size = bytes.len() as u64;
        let allocated = self
            .buffer
            .as_ref()
            .is_none_or(|buffer| buffer.size() < size);
        if allocated {
            self.buffer = Some(
                device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some(label),
                    size: size
                        .next_power_of_two()
                        .min(device.limits().max_buffer_size),
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }),
            );
        }
        let buffer = self.buffer.as_ref().expect("vertex buffer allocated");
        let range = if allocated || bytes.len() > MAX_VERTEX_SHADOW_BYTES {
            0..bytes.len()
        } else {
            changed_vertex_range(&self.shadow, bytes)
        };
        if !range.is_empty() {
            uploads.write_buffer(device, buffer, range.start as u64, &bytes[range.clone()]);
        }
        if bytes.len() <= MAX_VERTEX_SHADOW_BYTES {
            if self.shadow.capacity() < bytes.len() {
                self.shadow.reserve_exact(bytes.len() - self.shadow.len());
            }
            self.shadow.resize(bytes.len(), 0);
            self.shadow[range.clone()].copy_from_slice(&bytes[range]);
        } else {
            self.shadow = Vec::new();
        }
        Some(buffer.clone())
    }
}

#[derive(Default)]
pub(crate) struct FragmentBuffers {
    pub(crate) solid: VertexBuffer,
    pub(crate) scene: VertexBuffer,
    pub(crate) analytic: VertexBuffer,
    pub(crate) extended: VertexBuffer,
    pub(crate) clip: VertexBuffer,
    pub(crate) text: VertexBuffer,
}

/// Compare aligned chunks, then tighten the changed interval to copy alignment.
/// Suffix comparisons use the same offsets even when a stream grows or shrinks.
fn changed_vertex_range(previous: &[u8], next: &[u8]) -> std::ops::Range<usize> {
    const CHUNK: usize = 64;
    const ALIGN: usize = wgpu::COPY_BUFFER_ALIGNMENT as usize;
    debug_assert_eq!(next.len() % ALIGN, 0);
    let common = previous.len().min(next.len());
    let mut first = 0;
    while first + CHUNK <= common && previous[first..first + CHUNK] == next[first..first + CHUNK] {
        first += CHUNK;
    }
    while first < common && previous[first] == next[first] {
        first += 1;
    }
    if first == next.len() {
        return first..first;
    }
    let mut last = next.len();
    if next.len() <= previous.len() {
        while last >= first + CHUNK && previous[last - CHUNK..last] == next[last - CHUNK..last] {
            last -= CHUNK;
        }
        while last > first && previous[last - 1] == next[last - 1] {
            last -= 1;
        }
    }
    first / ALIGN * ALIGN..last.div_ceil(ALIGN) * ALIGN
}

#[cfg(test)]
mod tests {
    use super::changed_vertex_range;

    #[test]
    fn dirty_range_handles_alignment_growth_shrink_and_multiple_edits() {
        let previous = vec![7; 256];
        assert_eq!(changed_vertex_range(&previous, &previous), 256..256);
        assert_eq!(changed_vertex_range(&previous, &previous[..128]), 128..128);
        assert_eq!(changed_vertex_range(&previous[..128], &previous), 128..256);
        let mut changed = previous.clone();
        changed[69] = 8;
        assert_eq!(changed_vertex_range(&previous, &changed), 68..72);
        changed[200] = 8;
        assert_eq!(changed_vertex_range(&previous, &changed), 68..204);
        assert_eq!(changed_vertex_range(&previous, &changed[..128]), 68..72);
        assert_eq!(changed_vertex_range(&[], &previous), 0..256);
        assert_eq!(changed_vertex_range(&previous, &[]), 0..0);
    }

    #[test]
    fn large_streams_drop_the_cpu_shadow_and_small_streams_resume_delta_uploads() {
        use super::{GpuUploads, MAX_VERTEX_SHADOW_BYTES, VertexBuffer};
        let mut renderer = crate::WgpuRenderer::new();
        renderer
            .render(&sui_scene::SceneFrame::new(
                sui_core::WindowId::new(8958),
                sui_core::Size::new(1.0, 1.0),
            ))
            .unwrap();
        let shared = renderer.shared.as_ref().unwrap();
        let mut uploads = GpuUploads::default();
        let mut buffer = VertexBuffer::default();
        let large = vec![1u32; MAX_VERTEX_SHADOW_BYTES / 4 + 1];
        buffer.upload(&shared.device, &mut uploads, "oversized stream", &large);
        assert_eq!(buffer.shadow.capacity(), 0);
        let small = [3u32; 64];
        buffer.upload(&shared.device, &mut uploads, "small stream", &small);
        assert!(buffer.shadow.capacity() <= MAX_VERTEX_SHADOW_BYTES);
        let written = uploads.bytes_written;
        buffer.upload(&shared.device, &mut uploads, "unchanged stream", &small);
        assert_eq!(uploads.bytes_written, written);
        shared.queue.submit(uploads.finish());
    }
}
