//! Non-blocking GPU-to-CPU copies of the program frame for network outputs.
//!
//! The renderer never waits on a readback. Each captured frame goes into one
//! of a small ring of staging buffers. Mapping starts after the copy's
//! submission and is polled without blocking, so a frame is delivered one or
//! two frames after it was rendered. When every buffer is still in flight the
//! capture is skipped and counted instead of stalling the render loop.

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

use crate::PROGRAM_FORMAT;

/// Staging buffers in flight; three covers a frame of mapping latency.
const SLOTS: usize = 3;

const FREE: u8 = 0;
const COPIED: u8 = 1;
const MAPPING: u8 = 2;
const MAPPED: u8 = 3;
const FAILED: u8 = 4;

struct Slot {
    buffer: wgpu::Buffer,
    state: Arc<AtomicU8>,
    sequence: u64,
}

/// One delivered frame: RGBA8 sRGB rows, `stride` bytes apart.
pub struct ReadbackFrame<'a> {
    pub data: &'a [u8],
    pub extent: [u32; 2],
    pub stride: u32,
    pub sequence: u64,
}

pub struct ProgramReadback {
    slots: Vec<Slot>,
    extent: [u32; 2],
    stride: u32,
    next_sequence: u64,
    skipped: u64,
}

impl ProgramReadback {
    pub fn new(device: &wgpu::Device, extent: [u32; 2]) -> Self {
        let extent = [extent[0].max(1), extent[1].max(1)];
        debug_assert_eq!(PROGRAM_FORMAT.block_copy_size(None), Some(4));
        // Copies require 256-byte row alignment; receivers accept a stride.
        let stride = (extent[0] * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let slots = (0..SLOTS)
            .map(|index| Slot {
                buffer: device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some(&format!("virtual-program-readback-{index}")),
                    size: u64::from(stride) * u64::from(extent[1]),
                    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                    mapped_at_creation: false,
                }),
                state: Arc::new(AtomicU8::new(FREE)),
                sequence: 0,
            })
            .collect();
        Self {
            slots,
            extent,
            stride,
            next_sequence: 0,
            skipped: 0,
        }
    }

    pub fn extent(&self) -> [u32; 2] {
        self.extent
    }

    /// Captures skipped because every staging buffer was still in flight.
    pub fn skipped(&self) -> u64 {
        self.skipped
    }

    /// Records a copy of `texture` (the program target) into a free slot.
    /// Returns false, without recording anything, when no slot is free.
    pub fn capture(&mut self, encoder: &mut wgpu::CommandEncoder, texture: &wgpu::Texture) -> bool {
        debug_assert_eq!(texture.format(), PROGRAM_FORMAT);
        debug_assert_eq!([texture.width(), texture.height()], self.extent);
        let Some(slot) = self
            .slots
            .iter_mut()
            .find(|slot| slot.state.load(Ordering::Acquire) == FREE)
        else {
            self.skipped += 1;
            return false;
        };
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &slot.buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(self.stride),
                    rows_per_image: Some(self.extent[1]),
                },
            },
            wgpu::Extent3d {
                width: self.extent[0],
                height: self.extent[1],
                depth_or_array_layers: 1,
            },
        );
        slot.sequence = self.next_sequence;
        self.next_sequence += 1;
        slot.state.store(COPIED, Ordering::Release);
        true
    }

    /// Call after submitting the encoder that recorded captures. Starts
    /// mapping new copies, polls without blocking, and hands every finished
    /// frame to `deliver` in capture order.
    pub fn poll(&mut self, device: &wgpu::Device, mut deliver: impl FnMut(ReadbackFrame<'_>)) {
        for slot in &self.slots {
            if slot
                .state
                .compare_exchange(COPIED, MAPPING, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                let state = Arc::clone(&slot.state);
                slot.buffer
                    .slice(..)
                    .map_async(wgpu::MapMode::Read, move |result| {
                        state.store(
                            if result.is_ok() { MAPPED } else { FAILED },
                            Ordering::Release,
                        );
                    });
            }
        }
        let _ = device.poll(wgpu::PollType::Poll);
        let mut ready: Vec<usize> = (0..self.slots.len())
            .filter(|&index| {
                matches!(
                    self.slots[index].state.load(Ordering::Acquire),
                    MAPPED | FAILED
                )
            })
            .collect();
        ready.sort_by_key(|&index| self.slots[index].sequence);
        for index in ready {
            let slot = &self.slots[index];
            if slot.state.load(Ordering::Acquire) == MAPPED {
                {
                    let view = slot.buffer.slice(..).get_mapped_range();
                    deliver(ReadbackFrame {
                        data: &view,
                        extent: self.extent,
                        stride: self.stride,
                        sequence: slot.sequence,
                    });
                }
                slot.buffer.unmap();
            }
            slot.state.store(FREE, Ordering::Release);
        }
    }

    /// Blocks until every in-flight copy is delivered. For tests and
    /// shutdown; the render loop uses [`Self::poll`].
    pub fn flush(&mut self, device: &wgpu::Device, mut deliver: impl FnMut(ReadbackFrame<'_>)) {
        while self
            .slots
            .iter()
            .any(|slot| slot.state.load(Ordering::Acquire) != FREE)
        {
            self.poll(device, &mut deliver);
            let _ = device.poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: None,
            });
        }
    }
}
