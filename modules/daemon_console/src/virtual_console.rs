use alloc::borrow::ToOwned;
use alloc::collections::VecDeque;
use alloc::string::String;
use alloc::vec::Vec;
use core::ptr::Unique;

use super::{keyboard::EventAction, vga};
use d7keymap::{KeyAction, KeySymbol};

/// Screen and scrollback history
#[derive(Debug, Clone)]
pub struct Output {
    lines: VecDeque<Vec<u8>>,
    height: usize,
    width: usize,
}
impl Output {
    pub fn new() -> Self {
        Self {
            lines: VecDeque::new(),
            height: 25,
            width: 80,
        }
    }

    pub fn write_str(&mut self, text: &[u8]) {
        for byte in text {
            self.write_byte(*byte);
        }
    }

    pub fn write_byte(&mut self, byte: u8) {
        assert!(byte != 0);

        if byte == b'\r' {
            return;
        }

        if byte == b'\n' {
            self.new_line();
            return;
        }

        // Ensure that there is a last line and it has space for one character
        if let Some(last_line) = self.lines.back_mut() {
            if last_line.len() == self.width {
                self.new_line();
            }
        } else {
            self.lines.push_back(Vec::new());
        };

        self.lines.back_mut().unwrap().push(byte);
    }

    pub fn new_line(&mut self) {
        self.lines.push_back(Vec::new());
        if self.lines.len() > self.height {
            self.lines.pop_front();
        }
    }

    /// Render to vga buffer
    pub fn render(&mut self, buffer: &mut Unique<vga::Buffer>) {
        let color = vga::CellColor::new(vga::Color::White, vga::Color::Black);
        let mut it = self.lines.iter().rev().take(25).rev();
        let empty = Vec::new();
        for line in 0..self.height {
            let text = it.next().unwrap_or(&empty);
            for column in 0..self.width {
                let character = *text.get(column).unwrap_or(&b' ');
                unsafe {
                    buffer.as_mut().chars[line][column].write(vga::CharCell { character, color });
                }
            }
        }
    }
}

/// Keyboard input
#[derive(Debug, Clone)]
pub struct Input {
    dead_key_buffer: String,
    input_buffer: String,
}
impl Input {
    pub fn new() -> Self {
        Self {
            dead_key_buffer: String::new(),
            input_buffer: String::new(),
        }
    }

    pub fn keyboard_event(&mut self, action: EventAction) {
        use unicode_normalization::UnicodeNormalization;
        use unicode_segmentation::UnicodeSegmentation;

        match action {
            EventAction::KeyAction(action) => match action {
                KeyAction::Text(text) => {
                    if !self.dead_key_buffer.is_empty() {
                        self.input_buffer.extend(self.dead_key_buffer.drain(..));
                    }
                    self.input_buffer.push_str(&text);
                    self.input_buffer = self.input_buffer.nfc().collect();
                },
                KeyAction::Buffer(text) => {
                    self.dead_key_buffer.push_str(&text);
                },
                KeyAction::Remap(_) => unreachable!(),
                KeyAction::Ignore => {},
            },
            EventAction::Unmatched(symbol, modifiers) => match symbol.as_str() {
                "Enter" if modifiers.is_empty() => {
                    self.input_buffer.push('\n');
                },
                "Backspace" if modifiers.is_empty() => {
                    self.dead_key_buffer.clear();
                    let mut c: Vec<_> =
                        UnicodeSegmentation::graphemes(self.input_buffer.as_str(), true).collect();
                    c.pop();
                    self.input_buffer = c.join("");
                },
                _ => {},
            },
            EventAction::Ignore | EventAction::NoSuchSymbol => {},
        }
    }
}

#[derive(Debug)]
pub struct VirtualConsole {
    pub output: Output,
    pub input: Input,
}
impl VirtualConsole {
    pub fn new() -> Self {
        Self {
            output: Output::new(),
            input: Input::new(),
        }
    }

    pub fn render(&mut self, buffer: &mut Unique<vga::Buffer>) {
        // Build last line from the input and last line
        let mut s = self.output.clone();
        s.write_str(&self.input.input_buffer.as_bytes());
        s.render(buffer);
    }
}
