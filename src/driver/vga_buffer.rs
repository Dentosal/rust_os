use core::mem;
use core::ptr::NonNull;
use core::sync::atomic::{AtomicU64, Ordering};
use spin::Mutex;
use volatile::Volatile;

use crate::memory::prelude::{PhysAddr, VirtAddr};

const SCREEN_HEIGHT: usize = 25;
const SCREEN_WIDTH: usize = 80;
pub const VGA_BUFFER_ADDR_U64: u64 = 0xb8000;
pub const VGA_BUFFER_PHYSADDR: PhysAddr = unsafe { PhysAddr::new_unchecked(VGA_BUFFER_ADDR_U64) };
pub const VGA_BUFFER_VIRTADDR: VirtAddr = unsafe { VirtAddr::new_unsafe(VGA_BUFFER_ADDR_U64) };

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
#[repr(C, packed)]
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

/// A Virtual tty buffer
pub struct Buffer {
    pub chars: [[Volatile<CharCell>; SCREEN_WIDTH]; SCREEN_HEIGHT],
}

/// Cursor
pub struct Cursor {
    /// Current row
    row_col: AtomicU64,
}

impl Cursor {
    pub const fn new() -> Self {
        Self {
            row_col: AtomicU64::new(0),
        }
    }

    /// Next character, returns true on "overflow"
    pub fn next_col(&self) -> bool {
        let mut overflow = false;
        self.update_position(|mut row, mut col| {
            if col + 1 < SCREEN_WIDTH {
                col += 1;
            } else {
                overflow = true;
            }
            (row, col)
        });
        overflow
    }

    /// Previous character, goes to previous line if needed
    pub fn prev(&self) {
        self.update_position(|mut row, mut col| {
            if col > 0 {
                col -= 1;
            } else if row > 0 {
                row -= 1;
                col = SCREEN_WIDTH - 1;
            }
            (row, col)
        });
    }

    /// New line
    /// # Returns
    /// true if terminal should be scrolled
    pub fn newline(&self) -> bool {
        let mut result = false;
        self.update_position(|mut row, _col| {
            if row < SCREEN_HEIGHT - 1 {
                row += 1;
                result = false;
            } else {
                result = true;
            }
            (row, 0)
        });
        result
    }

    pub fn set_col(&self, col: usize) {
        assert!(col < SCREEN_WIDTH);

        self.row_col
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |mut value| {
                Some((value & 0xffff_ffff_0000_0000) | (col as u64))
            })
            .unwrap();
    }

    pub fn set_position(&self, row: usize, col: usize) {
        assert!(row < SCREEN_HEIGHT);
        assert!(col < SCREEN_WIDTH);

        self.row_col
            .store(((row as u64) << 32) | (col as u64), Ordering::SeqCst);
    }

    fn update_position<F>(&self, mut f: F) -> (usize, usize)
    where F: FnMut(usize, usize) -> (usize, usize) {
        let value = self
            .row_col
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |mut value| {
                let (row, col) = ((value >> 32) as usize, (value & 0xffff_ffff) as usize);
                let (new_row, new_col) = f(row, col);
                assert!(new_row < SCREEN_HEIGHT);
                assert!(new_col < SCREEN_WIDTH);
                Some(((new_row as u64) << 32) | (new_col as u64))
            })
            .unwrap();
        ((value >> 32) as usize, (value & 0xffff_ffff) as usize)
    }

    #[inline]
    pub fn position(&self) -> (usize, usize) {
        let value = self.row_col.load(Ordering::SeqCst);
        ((value >> 32) as usize, (value & 0xffff_ffff) as usize)
    }
}

/// Terminal: an interface to hardware terminal
pub struct Terminal<'a> {
    cursor: &'a Cursor,
    buffer: VirtAddr,
}

impl Terminal<'static> {
    /// Init terminal
    pub fn reset(&self) {
        self.clear(CellColor::new(Color::White, Color::Black));
    }

    /// Clear screen
    pub fn clear(&self, color: CellColor) {
        self.cursor.set_position(0, 0);
        let mut b = self.get_buffer();
        let buffer = unsafe { b.as_mut() };
        for col in 0..SCREEN_WIDTH {
            for row in 0..SCREEN_HEIGHT {
                buffer.chars[row][col].write(CharCell {
                    character: b' ',
                    color,
                });
            }
        }
    }

    /// Clear the current line
    pub fn clear_line(&self, color: CellColor) {
        let (row, _) = self.cursor.position();
        let mut b = self.get_buffer();
        let buffer = unsafe { b.as_mut() };
        for col in 0..SCREEN_WIDTH {
            buffer.chars[row][col].write(CharCell {
                character: b' ',
                color,
            });
        }
        self.cursor.set_col(0);
    }

    /// Write single byte to terminal's stdout
    pub fn write_byte(&self, byte: u8, color: CellColor) {
        assert!(byte != 0);

        let mut b = self.get_buffer();
        let buffer = unsafe { b.as_mut() };

        if byte == b'\n' {
            self.newline();
        } else if byte == 0x8 {
            // ASCII backspace
            self.backspace();
        } else {
            // assert!(self.cursor.col < SCREEN_WIDTH);

            let (row, col) = self.cursor.position();

            buffer.chars[row][col].write(CharCell {
                character: byte,
                color,
            });

            let col_overflows = self.cursor.next_col();
            if col_overflows {
                self.newline();
            }
        }
    }

    /// Newline
    pub fn newline(&self) {
        let scroll = self.cursor.newline();
        if scroll {
            self.scroll_line();
        }
    }

    /// Backspace
    pub fn backspace(&self) {
        self.cursor.prev();
        self.write_byte(b' ', CellColor::new(Color::White, Color::Black));
        self.cursor.prev();
    }

    /// Scroll up one line
    fn scroll_line(&self) {
        // move lines up
        let mut b = self.get_buffer();
        let buffer = unsafe { b.as_mut() };
        for row in 0..(SCREEN_HEIGHT - 1) {
            for col in 0..SCREEN_WIDTH {
                let new_value = buffer.chars[row + 1][col].read();
                buffer.chars[row][col].write(new_value);
            }
        }
        // clear the bottom line
        for col in 0..SCREEN_WIDTH {
            buffer.chars[SCREEN_HEIGHT - 1][col].write(CharCell {
                character: b' ',
                color: CellColor::new(Color::White, Color::Black),
            });
        }
    }

    /// Get pointer to memory buffer
    fn get_buffer(&self) -> NonNull<Buffer> {
        unsafe { NonNull::new(self.buffer.as_mut_ptr()).unwrap() }
    }
}

/// Allow formatting
impl ::core::fmt::Write for Terminal<'static> {
    fn write_str(&mut self, s: &str) -> ::core::fmt::Result {
        for byte in s.bytes() {
            self.write_byte(byte, CellColor::new(Color::White, Color::Black))
        }
        Ok(()) // Success. Always.
    }
}

static CURSOR: Cursor = Cursor::new();

pub fn hardware_terminal() -> Terminal<'static> {
    Terminal {
        cursor: &CURSOR,
        buffer: VGA_BUFFER_VIRTADDR,
    }
}

/// Actual print function. This should only be externally called using the rprint! and rprintln! macros.
pub fn print(fmt: ::core::fmt::Arguments) {
    use core::fmt::Write;
    let mut t = hardware_terminal();
    t.write_fmt(fmt).expect("??");
}

/// "Raw" output macros
macro_rules! rprint {
    ($($arg:tt)*) => ({
        $crate::driver::vga_buffer::print(format_args!($($arg)*));
    });
}
macro_rules! rprintln {
    ($fmt:expr) => (rprint!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => (rprint!(concat!($fmt, "\n"), $($arg)*));
}
macro_rules! rforce_unlock {
    () => {{}};
}
macro_rules! rreset {
    () => {{
        $crate::driver::vga_buffer::hardware_terminal().reset();
    }};
}
macro_rules! panic_indicator {
    ($x:expr) => ({
        ::core::arch::asm!(
            concat!("mov eax, ", stringify!($x), "; mov [0xb809c], eax"),
            out("eax") _,
            options(nostack)
        );
    });
    () => ({
        panic_indicator!(0x4f214f70);   // !p
    });
}
