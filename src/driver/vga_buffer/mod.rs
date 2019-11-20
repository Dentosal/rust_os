use core::mem;
use core::ptr::Unique;
use spin::Mutex;
use volatile::Volatile;

use crate::memory::prelude::{PhysAddr, VirtAddr};

const SCREEN_HEIGHT: usize = 25;
const SCREEN_WIDTH: usize = 80;
pub const VGA_BUFFER_ADDR_U64: u64 = 0xb8000;
pub const VGA_BUFFER_PHYSADDR: PhysAddr = unsafe { PhysAddr::new_unchecked(VGA_BUFFER_ADDR_U64) };
pub const VGA_BUFFER_VIRTADDR: VirtAddr =
    unsafe { VirtAddr::new_unchecked_raw(VGA_BUFFER_ADDR_U64) };

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

/// A Virtual tty buffer
pub struct Buffer {
    pub chars: [[Volatile<CharCell>; SCREEN_WIDTH]; SCREEN_HEIGHT],
}

/// Cursor
pub struct Cursor {
    /// Current row
    pub row: usize,
    /// Current column
    pub col: usize,
}

impl Cursor {
    /// Next character
    pub fn next(&mut self) {
        if self.col < SCREEN_WIDTH {
            self.col += 1;
        }
    }

    /// Previous character
    /// if row is true, goes to previous line if needed
    pub fn prev(&mut self, row: bool) {
        if self.col > 0 {
            self.col -= 1;
        } else if row && self.row > 0 {
            self.row -= 1;
            self.col = SCREEN_WIDTH - 1;
        }
    }

    /// New line
    /// # Returns
    /// true if terminal should be scrolled
    pub fn newline(&mut self) -> bool {
        self.col = 0;
        if self.row < SCREEN_HEIGHT - 1 {
            self.row += 1;
            false
        } else {
            true
        }
    }

    /// Set position
    pub fn set_position(&mut self, row: usize, col: usize) {
        assert!(row < SCREEN_HEIGHT);
        assert!(col < SCREEN_WIDTH);

        self.row = row;
        self.col = col;
    }

    pub fn position(&self) -> (usize, usize) {
        (self.row, self.col)
    }
}

/// Terminal: an interface to hardware terminal
pub struct Terminal {
    output_color: CellColor,
    cursor: Cursor,
    buffer: Unique<Buffer>,
}

impl Terminal {
    /// Init terminal
    pub fn reset(&mut self) {
        self.output_color = CellColor::new(Color::White, Color::Black);
        self.clear();
    }

    /// Clear screen
    pub fn clear(&mut self) {
        self.cursor.set_position(0, 0);
        let clear_color = self.output_color;
        let buffer = self.get_buffer();
        for col in 0..SCREEN_WIDTH {
            for row in 0..SCREEN_HEIGHT {
                buffer.chars[row][col].write(CharCell {
                    character: b' ',
                    color: clear_color,
                });
            }
        }
    }

    /// Write single byte to terminal's stdout
    pub fn write_byte(&mut self, byte: u8) {
        assert!(byte != 0);

        if byte == b'\n' {
            self.newline();
        } else if byte == 0x8 {
            // ASCII backspace
            self.backspace();
        } else {
            assert!(self.cursor.col < SCREEN_WIDTH);

            let color = self.output_color;
            let (row, col) = self.cursor.position();

            self.get_buffer().chars[row][col].write(CharCell {
                character: byte,
                color,
            });

            self.cursor.next();

            if self.cursor.col >= SCREEN_WIDTH {
                self.newline();
            }
        }
    }

    /// Get color
    pub fn get_color(&mut self) -> CellColor {
        self.output_color
    }

    /// Set color
    pub fn set_color(&mut self, color: CellColor) {
        self.output_color = color;
    }

    /// Newline
    pub fn newline(&mut self) {
        let scroll = self.cursor.newline();
        if scroll {
            self.scroll_line();
        }
    }

    /// Backspace
    pub fn backspace(&mut self) {
        self.cursor.prev(true);
        self.write_byte(b' ');
        self.cursor.prev(true);
    }

    /// Scroll up one line
    fn scroll_line(&mut self) {
        // move lines up
        for row in 0..(SCREEN_HEIGHT - 1) {
            for col in 0..SCREEN_WIDTH {
                let new_value = self.get_buffer().chars[row + 1][col].read();
                self.get_buffer().chars[row][col].write(new_value);
            }
        }
        // clear the bottom line
        for col in 0..SCREEN_WIDTH {
            let color = self.output_color;
            self.get_buffer().chars[SCREEN_HEIGHT - 1][col].write(CharCell {
                character: b' ',
                color,
            });
        }
    }

    /// Get pointer to memory buffer
    fn get_buffer(&mut self) -> &mut Buffer {
        unsafe { self.buffer.as_mut() }
    }
}

/// Allow formatting
impl ::core::fmt::Write for Terminal {
    fn write_str(&mut self, s: &str) -> ::core::fmt::Result {
        for byte in s.bytes() {
            self.write_byte(byte)
        }
        Ok(()) // Success. Always.
    }
}

/// Actual print function. This should only be externally called using the rprint! and rprintln! macros.
pub fn print(fmt: ::core::fmt::Arguments) {
    use core::fmt::Write;
    TERMINAL.lock().write_fmt(fmt).unwrap();
}

/// Print with color
pub fn printc(fmt: ::core::fmt::Arguments, color: CellColor) {
    use core::fmt::Write;
    let mut t = TERMINAL.lock();
    let old_color = t.get_color();
    t.set_color(color);
    t.write_fmt(fmt).unwrap();
    t.set_color(old_color);
}

// Create static pointer mutex with spinlock to make TERMINAL thread-safe
pub static TERMINAL: Mutex<Terminal> = Mutex::new(Terminal {
    output_color: CellColor::new(Color::White, Color::Black),
    cursor: Cursor { row: 0, col: 0 },
    buffer: unsafe { Unique::new_unchecked(VGA_BUFFER_VIRTADDR.as_mut_ptr_unchecked()) },
});

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
macro_rules! rprintc {
    ($fg:expr, $bg:expr ; $($arg:tt)*) => ({
        use $crate::driver::vga_buffer::CellColor;
        $crate::driver::vga_buffer::printc(
            format_args!($($arg)*),
            CellColor::new($fg, $bg)
        );
    });

}
macro_rules! rprintlnc {
    ($fg:expr, $bg:expr ; $fmt:expr, $($arg:tt)*) => (rprintc!( $fg, $bg; concat!($fmt, "\n"), $($arg)*));
    ($fg:expr ; $fmt:expr, $($arg:tt)*) => (rprintlnc!($fg, Color::Black ; $fmt, $($arg)*));
    ($fg:expr, $bg:expr ; $fmt:expr) => (rprintc!( $fg, $bg ; concat!($fmt, "\n")));
    ($fg:expr ; $fmt:expr) => (rprintlnc!($fg, Color::Black ; $fmt));
}
macro_rules! rreset {
    () => {{
        $crate::driver::vga_buffer::TERMINAL.lock().reset();
    }};
}
macro_rules! rforce_unlock {
    () => {{
        $crate::driver::vga_buffer::TERMINAL.force_unlock();
    }};
}
macro_rules! panic_indicator {
    ($x:expr) => ({
        asm!(concat!("mov eax, ", stringify!($x), "; mov [0xb809c], eax") ::: "eax", "memory" : "volatile", "intel");
    });
    () => ({
        panic_indicator!(0x4f214f70);   // !p
    });
}
