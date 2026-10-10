use virtual_core::ProjectionMapping;
use virtual_render::{PROGRAM_FORMAT, PresentationOptions, ProgramPresenter, ProgramTarget};

#[test]
fn projection_crops_warps_masks_and_preserves_the_unmapped_program() {
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let Ok(adapter) =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
    else {
        eprintln!("no GPU adapter; skipping projection readback");
        return;
    };
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
    let program = ProgramTarget::new(&device, [64, 64]);
    let presenter = ProgramPresenter::new(&device, &program, PROGRAM_FORMAT);
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(
            r#"
            @vertex fn vs(@builtin(vertex_index) i:u32)->@builtin(position) vec4<f32> {
                let p=array(vec2(-1.0,-1.0),vec2(3.0,-1.0),vec2(-1.0,3.0));
                return vec4(p[i],0.0,1.0);
            }
            @fragment fn fs(@builtin(position) p:vec4<f32>)->@location(0) vec4<f32> {
                if p.x < 32.0 { return vec4(1.0,0.0,0.0,1.0); }
                return vec4(0.0,1.0,0.0,1.0);
            }
        "#
            .into(),
        ),
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: None,
        layout: None,
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs"),
            compilation_options: Default::default(),
            targets: &[Some(PROGRAM_FORMAT.into())],
        }),
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &program.view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&pipeline);
        pass.draw(0..3, 0..1);
    }
    queue.submit([encoder.finish()]);
    let output = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: 64,
            height: 64,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: PROGRAM_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = output.create_view(&Default::default());
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 256 * 64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let draw = |projection| {
        let mut encoder = device.create_command_encoder(&Default::default());
        presenter.draw(
            &queue,
            &mut encoder,
            &view,
            [64, 64],
            PresentationOptions {
                projection,
                ..Default::default()
            },
        );
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &output,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(256),
                    rows_per_image: Some(64),
                },
            },
            wgpu::Extent3d {
                width: 64,
                height: 64,
                depth_or_array_layers: 1,
            },
        );
        queue.submit([encoder.finish()]);
        readback
            .slice(..)
            .map_async(wgpu::MapMode::Read, |result| result.unwrap());
        device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: None,
            })
            .unwrap();
        let pixels = readback.slice(..).get_mapped_range().to_vec();
        readback.unmap();
        pixels
    };
    let mapping = ProjectionMapping {
        enabled: true,
        corners: [[0.25, 0.25], [0.75, 0.25], [0.9, 0.9], [0.1, 0.9]],
        source: [0.5, 0.0, 1.0, 1.0],
        masks: [[0.45, 0.45, 0.55, 0.55], [0.0; 4], [0.0; 4], [0.0; 4]],
        ..Default::default()
    };
    let pixels = draw(mapping);
    let pixel = |x: usize, y: usize| &pixels[(y * 64 + x) * 4..(y * 64 + x) * 4 + 4];
    assert_eq!(pixel(2, 2), [0, 0, 0, 255], "outside quad");
    assert_eq!(pixel(32, 32), [0, 0, 0, 255], "blackout mask");
    assert_eq!(
        pixel(20, 20),
        [0, 255, 0, 255],
        "cropped source inside quad"
    );
    assert_eq!(
        pixel(44, 44),
        [0, 255, 0, 255],
        "perspective-mapped lower corner"
    );
    let normal = draw(ProjectionMapping {
        enabled: false,
        ..mapping
    });
    assert_eq!(
        &normal[20 * 256 + 10 * 4..20 * 256 + 10 * 4 + 4],
        [255, 0, 0, 255]
    );
    assert_eq!(
        &normal[20 * 256 + 50 * 4..20 * 256 + 50 * 4 + 4],
        [0, 255, 0, 255]
    );
    let unfeathered = draw(ProjectionMapping {
        enabled: true,
        ..Default::default()
    });
    let feathered = draw(ProjectionMapping {
        enabled: true,
        edge_blend: [0.5, 0.0, 0.0, 0.0],
        ..Default::default()
    });
    let near_edge = (32 * 64 + 8) * 4;
    let farther_in = (32 * 64 + 24) * 4;
    assert!(
        feathered[near_edge] < unfeathered[near_edge]
            && feathered[farther_in] > feathered[near_edge],
        "edge feather should smoothly reduce output toward the selected edge"
    );
    let black_compensated = draw(ProjectionMapping {
        enabled: true,
        black_level: 0.1,
        ..Default::default()
    });
    assert!(
        black_compensated[near_edge + 2] > 70,
        "black-level compensation should lift dark color channels"
    );
    let invalid = draw(ProjectionMapping {
        corners: [[0.0; 2]; 4],
        ..mapping
    });
    assert!(
        invalid.chunks_exact(4).all(|pixel| pixel == [0, 0, 0, 255]),
        "invalid calibration must fail black"
    );
}
