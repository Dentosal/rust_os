use core::ptr::Unique;
use spin::Mutex;

use vga_buffer::{Color, CellColor, CharCell, Buffer};

const SCREEN_HEIGHT: usize = 25;
const SCREEN_WIDTH: usize = 80;

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
        if self.col < SCREEN_WIDTH-2 {
            self.col+=1;
        }
    }
    /// New line
    /// # Returns
    /// true if terminal should be scrolled
    pub fn newline(&mut self) -> bool {
        self.col = 0;
        self.row = 24;
        return true;
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
    /// Clear screen
    pub fn clear(&mut self) {
        self.cursor.set_position(0, 0);
        let clear_color = self.output_color;
        let mut buffer = self.get_buffer();
        for col in 0..SCREEN_WIDTH {
            for row in 0..SCREEN_HEIGHT {
                buffer.chars[row][col] = CharCell {
                    character: b'.',
                    color: clear_color,
                }
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
        self.cursor.newline();
        self.scroll_line();
    }

    /// Scroll up one line
    fn scroll_line(&mut self) {
        for row in 0..(SCREEN_HEIGHT-1) {
            self.get_buffer().chars[row] = self.get_buffer().chars[row+1];
        }
        self.get_buffer().chars[SCREEN_HEIGHT-1] = [CharCell {character: b' ', color: self.output_color}; SCREEN_WIDTH];
        // self.set_color(CellColor::new(Color::Red, Color::White));
        // self.write_byte(b'+');
    }

    /// Get pointer to memory buffer
    fn get_buffer(&mut self) -> &mut Buffer {
        unsafe {self.buffer.get_mut()}
    }
}


pub static TERMINAL: Mutex<Terminal> = Mutex::new(Terminal {
    raw_mode: false,
    // output_color: CellColor::new(Color::White, Color::Black),
    output_color: CellColor::new(Color::Black, Color::White),
    cursor: Cursor {row: 0, col: 0},
    buffer: unsafe { Unique::new(0xb8000 as *mut _) },
});
