use core::ptr::Unique;

use vga_buffer::{Color, CellColor, CharCell, Buffer};

const SCREEN_HEIGHT: usize = 25;
const SCREEN_WIDTH: usize = 80;

/// Cursor
pub struct Cursor {
    /// Current row
    row: usize,
    /// Current column
    col: usize,
}

impl Cursor {
    /// Next character
    pub fn next(&mut self) {
        if self.col < SCREEN_WIDTH-1 {
            self.col+=1;
        }
    }
    /// New line
    pub fn newline(&mut self) {
        self.col = 0;
        if self.row < SCREEN_HEIGHT-1 {
            self.row+=1;
        }
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
    /// Create new Terminal
    pub fn new() -> Terminal {
        Terminal {
            raw_mode: false,
            // output_color: CellColor::new(Color::White, Color::Black),
            output_color: CellColor::new(Color::Black, Color::White),
            cursor: Cursor {row: 0, col: 0},
            buffer: unsafe { Unique::new(0xb8000 as *mut _) },
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

    pub fn newline(&mut self) {
        self.cursor.newline();
    }

    fn get_buffer(&mut self) -> &mut Buffer {
        unsafe {self.buffer.get_mut()}
    }
}
