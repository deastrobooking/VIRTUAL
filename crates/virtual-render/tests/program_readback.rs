use virtual_render::{ProgramReadback, ProgramTarget};

fn device() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .ok()?;
    pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).ok()
}

/// A width whose RGBA rows are not 256-byte aligned exercises the stride.
const EXTENT: [u32; 2] = [70, 5];

fn pattern() -> Vec<u8> {
    (0..EXTENT[1])
        .flat_map(|y| (0..EXTENT[0]).flat_map(move |x| [x as u8, y as u8, (x + y) as u8, 255]))
        .collect()
}

#[test]
fn readback_returns_the_program_frame_and_skips_rather_than_stalls() {
    let Some((device, queue)) = device() else {
        eprintln!("no GPU adapter; skipping program readback");
        return;
    };
    let program = ProgramTarget::new(&device, EXTENT);
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: program.texture(),
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &pattern(),
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(EXTENT[0] * 4),
            rows_per_image: Some(EXTENT[1]),
        },
        wgpu::Extent3d {
            width: EXTENT[0],
            height: EXTENT[1],
            depth_or_array_layers: 1,
        },
    );
    let mut readback = ProgramReadback::new(&device, EXTENT);
    let mut encoder = device.create_command_encoder(&Default::default());
    // Three slots: the fourth capture in one frame must be skipped.
    for _ in 0..3 {
        assert!(readback.capture(&mut encoder, program.texture()));
    }
    assert!(!readback.capture(&mut encoder, program.texture()));
    assert_eq!(readback.skipped(), 1);
    queue.submit([encoder.finish()]);

    let mut frames = Vec::new();
    readback.flush(&device, |frame| {
        assert_eq!(frame.extent, EXTENT);
        assert_eq!(frame.stride, 512, "rows padded to 256-byte alignment");
        let packed: Vec<u8> = frame
            .data
            .chunks(frame.stride as usize)
            .flat_map(|row| row[..EXTENT[0] as usize * 4].to_vec())
            .collect();
        frames.push((frame.sequence, packed));
    });
    assert_eq!(
        frames
            .iter()
            .map(|(sequence, _)| *sequence)
            .collect::<Vec<_>>(),
        [0, 1, 2],
        "delivered once each, in capture order"
    );
    assert!(frames.iter().all(|(_, pixels)| *pixels == pattern()));

    // Every slot is free again once delivered.
    let mut encoder = device.create_command_encoder(&Default::default());
    assert!(readback.capture(&mut encoder, program.texture()));
    queue.submit([encoder.finish()]);
    let mut delivered = 0;
    readback.flush(&device, |_| delivered += 1);
    assert_eq!(delivered, 1);
}
