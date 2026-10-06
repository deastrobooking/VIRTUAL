//! Renders every pattern into one BMP contact sheet and prints per-pattern
//! timing at a performance resolution.
//!
//! `cargo run --release -p virtual-generate --example contact_sheet -- out.bmp`

use std::time::Instant;

use virtual_generate::{AudioBands, Generator, GeneratorSettings, RecursivePattern};

const TILE: [u32; 2] = [480, 270];
const COLUMNS: u32 = 4;

fn main() -> std::io::Result<()> {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "contact_sheet.bmp".to_owned());
    let rows = (RecursivePattern::ALL.len() as u32).div_ceil(COLUMNS);
    let [sheet_w, sheet_h] = [TILE[0] * COLUMNS, TILE[1] * rows];
    let mut sheet = vec![0u8; (sheet_w * sheet_h * 4) as usize];

    for (index, pattern) in RecursivePattern::ALL.into_iter().enumerate() {
        let settings = GeneratorSettings {
            pattern,
            resolution: TILE,
            transparent: false,
            rotate_speed: 0.0,
            tilt: -0.2,
            ..GeneratorSettings::default()
        };
        let mut generator = Generator::new(settings.clone());
        let mut tile = vec![0u8; (TILE[0] * TILE[1] * 4) as usize];
        generator.render(0.0, AudioBands::default(), &mut tile);
        let (col, row) = (index as u32 % COLUMNS, index as u32 / COLUMNS);
        for y in 0..TILE[1] {
            let src = (y * TILE[0] * 4) as usize;
            let dst = (((row * TILE[1] + y) * sheet_w + col * TILE[0]) * 4) as usize;
            sheet[dst..dst + (TILE[0] * 4) as usize]
                .copy_from_slice(&tile[src..src + (TILE[0] * 4) as usize]);
        }

        for (label, depth) in [("default", 0.6), ("max", 1.0)] {
            let mut generator = Generator::new(GeneratorSettings {
                depth,
                resolution: [1280, 720],
                ..settings.clone()
            });
            let mut frame = vec![0u8; 1280 * 720 * 4];
            generator.render(0.0, AudioBands::default(), &mut frame);
            let first = generator.stats();
            let started = Instant::now();
            for i in 1..=10 {
                generator.render(i as f64 / 60.0, AudioBands::default(), &mut frame);
            }
            let per_frame = started.elapsed().as_secs_f64() * 100.0;
            println!(
                "{:<24} {label:<7} depth {:>2}/{:<2} {:>7} segs  gen {:>6.2} ms  frame {:>6.2} ms",
                pattern.label(),
                first.depth,
                first.requested_depth,
                first.segments,
                first.generate_micros as f64 / 1000.0,
                per_frame,
            );
        }
    }
    write_bmp(&path, sheet_w, sheet_h, &sheet)
}

fn write_bmp(path: &str, width: u32, height: u32, rgba: &[u8]) -> std::io::Result<()> {
    let row = width * 3;
    let padded = row.div_ceil(4) * 4;
    let size = 54 + padded * height;
    let mut out = Vec::with_capacity(size as usize);
    out.extend(b"BM");
    out.extend(size.to_le_bytes());
    out.extend([0u8; 4]);
    out.extend(54u32.to_le_bytes());
    out.extend(40u32.to_le_bytes());
    out.extend(width.to_le_bytes());
    out.extend(height.to_le_bytes());
    out.extend(1u16.to_le_bytes());
    out.extend(24u16.to_le_bytes());
    out.extend([0u8; 24]);
    for y in (0..height).rev() {
        for x in 0..width {
            let p = ((y * width + x) * 4) as usize;
            out.extend([rgba[p + 2], rgba[p + 1], rgba[p]]);
        }
        out.extend(std::iter::repeat_n(0u8, (padded - row) as usize));
    }
    std::fs::write(path, out)
}
