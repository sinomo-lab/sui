//! Reusable destination buffers and staging storage for frame uploads.
use bytemuck::Pod;

#[derive(Default)]
pub(crate) struct GpuUploads {
    belt: Option<wgpu::util::StagingBelt>,
    encoder: Option<wgpu::CommandEncoder>,
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

#[derive(Default)]
pub(crate) struct VertexBuffer {
    pub(crate) buffer: Option<wgpu::Buffer>,
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
        if self
            .buffer
            .as_ref()
            .is_none_or(|buffer| buffer.size() < size)
        {
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
        uploads.write_buffer(device, buffer, 0, bytes);
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
