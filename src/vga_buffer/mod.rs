// TODO: cursor visibility (reverse?)

use core::ptr::Unique;
use spin::Mutex;
use volatile::Volatile;

const SCREEN_HEIGHT: usize = 25;
const SCREEN_WIDTH: usize = 80;
const VGA_BUFFER_ADDRESS: usize = 0xb8000;

/// A VGA color
#[allow(dead_code)]
#[repr(u8)]
pub enum Color {
    Black      = 0,
    Blue       = 1,
    Green      = 2,
    Cyan       = 3,
    Red        = 4,
    Magenta    = 5,
    Brown      = 6,
    LightGray  = 7,
    DarkGray   = 8,
    LightBlue  = 9,
    LightGreen = 10,
    LightCyan  = 11,
    LightRed   = 12,
    Pink       = 13,
    Yellow     = 14,
    White      = 15,
}

/// Color of single cell, back- and foreground
#[derive(Clone, Copy)]
pub struct CellColor(u8);

impl CellColor {
    /// Contructor
    pub const fn new(foreground: Color, background: Color) -> CellColor {
        CellColor((background as u8) << 4 | (foreground as u8))
    }
}

/// Character cell: one character and color in screen
#[derive(Clone, Copy)]
#[repr(C,packed)]
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
            self.col+=1;
        }
    }

    /// New line
    /// # Returns
    /// true if terminal should be scrolled
    pub fn newline(&mut self) -> bool {
        self.col = 0;
        if self.row < SCREEN_HEIGHT-1 {
            self.row += 1;
            false
        }
        else {
            true
        }
    }

    /// Set position
    pub fn set_position(&mut self, row: usize, col: usize) {
        // TODO: boundary check
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
        let mut buffer = self.get_buffer();
        for col in 0..SCREEN_WIDTH {
            for row in 0..SCREEN_HEIGHT {
                buffer.chars[row][col].write(CharCell {
                    character: b' ',
                    color: clear_color,
                });
            }
        }
    }

    /// Write string to terminal's stdout
    pub fn write_str(&mut self, string: &str) {
        for b in string.bytes() {
            self.write_byte(b);
        }
    }

    /// Write single byte to terminal's stdout
    pub fn write_byte(&mut self, byte: u8) {
        if byte == b'\n' {
            self.newline();
        }
        else {
            if self.cursor.col >= SCREEN_WIDTH {
                self.newline();
            }

            let color = self.output_color;
            let (row, col) = self.cursor.position();
            self.get_buffer().chars[row][col].write(CharCell {
                character: byte,
                color: color
            });
            self.cursor.next();
        }
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

    /// Scroll up one line
    fn scroll_line(&mut self) {
        for row in 0..(SCREEN_HEIGHT-1) {
            for col in 0..SCREEN_WIDTH {
                let new_value = self.get_buffer().chars[row+1][col].read();
                self.get_buffer().chars[row][col].write(new_value);
            }
        }
        for col in 0..SCREEN_WIDTH {
            let color = self.output_color;
            self.get_buffer().chars[SCREEN_HEIGHT-1][col].write(CharCell {character: b' ', color: color});
        }
    }

    /// Get pointer to memory buffer
    fn get_buffer(&mut self) -> &mut Buffer {
        unsafe {self.buffer.get_mut()}
    }

    /// Create unsafe panic terminal
    /// Outputs red-on-black text to the last lines of the terminal
    /// DEPRECATED, remove ASAP
    pub unsafe fn get_panic_access() -> Terminal {
        Terminal {
            output_color: CellColor::new(Color::Red, Color::Black),
            cursor: Cursor {row: SCREEN_HEIGHT-1, col: 0},
            buffer: Unique::new(VGA_BUFFER_ADDRESS as *mut _),
        }
    }
}

/// Allow formatting
impl ::core::fmt::Write for Terminal {
    fn write_str(&mut self, s: &str) -> ::core::fmt::Result {
        for byte in s.bytes() {
            self.write_byte(byte)
        }
        Ok(())  // Success. Always.
    }
}

/// Actual print function. This should only be externally called using the rprint! and rprintln! macros.
pub fn print(fmt: ::core::fmt::Arguments) {
    use core::fmt::Write;
    TERMINAL.lock().write_fmt(fmt).unwrap();
}

// Create static pointer mutex with spinlock to make TERMINAL thread-safe
pub static TERMINAL: Mutex<Terminal> = Mutex::new(Terminal {
    output_color: CellColor::new(Color::White, Color::Black),
    cursor: Cursor {row: 0, col: 0},
    buffer: unsafe { Unique::new(VGA_BUFFER_ADDRESS as *mut _) },
});

/// "Raw" output macros
macro_rules! rprint {
    ($($arg:tt)*) => ({
        $crate::vga_buffer::print(format_args!($($arg)*));
    });
}
macro_rules! rprintln {
    ($fmt:expr) => (rprint!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => (rprint!(concat!($fmt, "\n"), $($arg)*));
}
macro_rules! rreset {
    () => ({
        $crate::vga_buffer::TERMINAL.lock().reset();
    });
}
macro_rules! panic_indicator {
    ($x:expr) => ({
        asm!(concat!("mov eax, ", stringify!($x), "; mov [0xb809c], eax") ::: "eax", "memory" : "volatile", "intel");
    });
    () => ({
        panic_indicator!(0x4f214f70);   // !p
    });
}
