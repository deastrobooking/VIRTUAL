//! Steady-state generator stack timing at 1080p for one to four layers.
//!
//! `cargo run --release -p virtual-generate --example stack_bench`
//!
//! Geometry is rebuilt only when settings change, so the first frame is
//! excluded: it would overstate the per-frame cost. Run as an example rather
//! than a test; the test harness compiles the blend loop measurably slower.
use std::time::Instant;

use virtual_generate::{AudioBands, GeneratorSettings, GeneratorStack, GeneratorStackRenderer};

fn main() {
    const WARMUP: u32 = 10;
    const FRAMES: u32 = 60;
    for count in 1..=4 {
        let mut stack = GeneratorStack::new(GeneratorSettings {
            resolution: [1920, 1080],
            depth: 0.2,
            ..GeneratorSettings::default()
        });
        for _ in 1..count {
            stack.add_layer();
        }
        stack.normalize_output();
        let mut renderer = GeneratorStackRenderer::new(&stack);
        let mut out = vec![0; 1920 * 1080 * 4];
        let time = |frame: u32| f64::from(frame) / 60.0;
        for frame in 0..WARMUP {
            renderer.render(&stack, time(frame), AudioBands::default(), &mut out);
        }
        let start = Instant::now();
        for frame in WARMUP..WARMUP + FRAMES {
            renderer.render(&stack, time(frame), AudioBands::default(), &mut out);
        }
        let per_frame = start.elapsed().as_secs_f64() * 1000.0 / f64::from(FRAMES);
        println!("{count} layer(s), 1080p: {per_frame:.2} ms/frame");
    }
}
