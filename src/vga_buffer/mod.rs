const SCREEN_HEIGHT: usize = 25;
const SCREEN_WIDTH: usize = 80;

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

/// Virtual tty buffer
pub struct Buffer {
    pub chars: [[CharCell; SCREEN_WIDTH]; SCREEN_HEIGHT],
}
