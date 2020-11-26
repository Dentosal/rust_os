use core::mem;
use core::ptr::Unique;
use volatile::Volatile;

use libd7::{syscall, PhysAddr, VirtAddr};

const SCREEN_HEIGHT: usize = 25;
const SCREEN_WIDTH: usize = 80;
const HARDWARE_BUFFER_ADDR: u64 = 0xb8000;
const HARDWARE_BUFFER_SIZE: u64 = mem::size_of::<Buffer>() as u64;

/// Should be free to use. Check plan.md
const VIRTUAL_ADDR: VirtAddr = unsafe { VirtAddr::new_unsafe(0x10_0000_0000) };

/// A VGA color
#[allow(dead_code)]
#[repr(u8)]
pub enum Color {
    Black = 0,
    Blue = 1,
    Green = 2,
    Cyan = 3,
    Red = 4,
    Magenta = 5,
    Brown = 6,
    LightGray = 7,
    DarkGray = 8,
    LightBlue = 9,
    LightGreen = 10,
    LightCyan = 11,
    LightRed = 12,
    Pink = 13,
    Yellow = 14,
    White = 15,
}

/// Color of single cell, back- and foreground
#[derive(Clone, Copy)]
pub struct CellColor(u8);

impl CellColor {
    pub const fn new(foreground: Color, background: Color) -> CellColor {
        CellColor((background as u8) << 4 | (foreground as u8))
    }

    pub fn foreground(self) -> Color {
        unsafe { mem::transmute::<u8, Color>(self.0 & 0xf) }
    }

    pub fn background(self) -> Color {
        unsafe { mem::transmute::<u8, Color>((self.0 & 0xf0) >> 4) }
    }

    pub fn invert(self) -> CellColor {
        CellColor::new(self.background(), self.foreground())
    }
}

/// Character cell: one character and color in screen
#[derive(Clone, Copy)]
#[repr(C, packed)]
pub struct CharCell {
    pub character: u8,
    pub color: CellColor,
}

#[repr(C, packed)]
pub struct Buffer {
    pub chars: [[Volatile<CharCell>; SCREEN_WIDTH]; SCREEN_HEIGHT],
}
impl Buffer {
    /// Clear screen
    pub fn clear(&mut self) {
        let color = CellColor::new(Color::White, Color::Black);
        for col in 0..SCREEN_WIDTH {
            for row in 0..SCREEN_HEIGHT {
                self.chars[row][col].write(CharCell {
                    character: b' ',
                    color,
                });
            }
        }
    }
}

/// # Safety
/// Must be only called once. Modifies kernel page tables.
pub unsafe fn get_hardware_buffer() -> Unique<Buffer> {
    syscall::mmap_physical(
        // Assumes 2MiB pages, so that 0xb8000 falls on the first page
        PhysAddr::new(0),
        VIRTUAL_ADDR,
        HARDWARE_BUFFER_SIZE,
        syscall::MemoryProtectionFlags::READ | syscall::MemoryProtectionFlags::WRITE,
    )
    .unwrap();
    Unique::new_unchecked((VIRTUAL_ADDR + HARDWARE_BUFFER_ADDR).as_mut_ptr())
}
