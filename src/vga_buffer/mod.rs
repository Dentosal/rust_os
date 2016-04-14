use core::ptr::Unique;
use spin::Mutex;

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
#[repr(C)]
pub struct CharCell {
    pub character: u8,
    pub color: CellColor,
}

/// A Virtual tty buffer
pub struct Buffer {
    pub chars: [[CharCell; SCREEN_WIDTH]; SCREEN_HEIGHT],
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
}

/// Terminal: an interface to hardware terminal
pub struct Terminal {
    raw_mode: bool,
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
                buffer.chars[row][col] = CharCell {
                    character: b' ',
                    color: clear_color,
                };
            }
        }
    }
    /// Write string to terminal's stdout
    pub fn write_str(&mut self, string: &str) {
        let sb = string.as_bytes();
        for index in 0..sb.len() {
            self.write_byte(sb[index]);
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

            self.get_buffer().chars[self.cursor.row][self.cursor.col] = CharCell {
                character: byte,
                color: self.output_color,
            };
            self.cursor.next();
        }
    }
    /// Set color
    pub fn set_color(&mut self, color: CellColor) {
        self.output_color = color;
    }

    /// Newline
    pub fn newline(&mut self) {
        if self.cursor.newline() {
            self.scroll_line();
        }
    }

    /// Scroll up one line
    fn scroll_line(&mut self) {
        for row in 0..(SCREEN_HEIGHT-1) {
            self.get_buffer().chars[row] = self.get_buffer().chars[row+1];
        }
        self.get_buffer().chars[SCREEN_HEIGHT-1] = [CharCell {character: b' ', color: self.output_color}; SCREEN_WIDTH];
    }

    /// Get pointer to memory buffer
    fn get_buffer(&mut self) -> &mut Buffer {
        unsafe {self.buffer.get_mut()}
    }
}
/// Format macros
impl ::core::fmt::Write for Terminal {
    fn write_str(&mut self, s: &str) -> ::core::fmt::Result {
        for byte in s.bytes() {
            self.write_byte(byte)
        }
        Ok(())  // Success. Always.
    }
}

// Create static pointer mutex with spinlock to make TERMINAL thread-safe
pub static TERMINAL: Mutex<Terminal> = Mutex::new(Terminal {
    raw_mode: false,
    output_color: CellColor::new(Color::White, Color::Black),
    cursor: Cursor {row: 0, col: 0},
    buffer: unsafe { Unique::new(VGA_BUFFER_ADDRESS as *mut _) },
});


/// "Raw" output macros
macro_rules! rprint {
    ($($arg:tt)*) => ({
        use core::fmt::Write;
        $crate::vga_buffer::TERMINAL.lock().write_fmt(format_args!($($arg)*)).unwrap();
    });
}
macro_rules! rprintln {
    ($fmt:expr) => (rprint!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => (rprint!(concat!($fmt, "\n"), $($arg)*));
}
macro_rules! rreset {
    () => ({
        use core::fmt::Write;
        $crate::vga_buffer::TERMINAL.lock().reset();
    });
}
