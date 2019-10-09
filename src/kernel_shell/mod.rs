use alloc::borrow::ToOwned;
use alloc::string::String;

use keyboard;

pub fn wait_enter() {
    loop {
        if let Some(event) = { (*keyboard::KEYBOARD.lock()).pop_event() } {
            if event.event_type == keyboard::KeyboardEventType::Press {
                if event.key == keyboard::Key::Enter {
                    break;
                }
            }
        }
    }
}

fn read_line() -> String {
    let mut buffer = String::new();
    loop {
        if let Some(event) = { (*keyboard::KEYBOARD.lock()).pop_event() } {
            if event.event_type == keyboard::KeyboardEventType::Press {
                if event.key == keyboard::Key::Backspace {
                    if !buffer.is_empty() {
                        rprint!("\u{8}");
                        buffer.pop();
                    }
                } else if let Some(c) = event.key.produces_text() {
                    if c == "\n".to_owned() {
                        return buffer;
                    }
                    rprint!("{}", c);
                    buffer.push_str(&c);
                }
            }
        }
    }
}

pub fn run() {
    loop {
        rprint!("$ ");
        let line = read_line();
        let line = line.trim();
        rprintln!("");
        rprintln!("{}", line);

        if line == "exit" {
            break;
        }

        if let Ok(line) = line.parse::<u64>() {
            let data = unsafe { crate::disk_io::DISK_IO.lock().read(line, 1)[0].clone() };
            let mut iter = data.iter();

            'outer: loop {
                for _ in 0..2 {
                    for _ in 0..8 {
                        if let Some(x) = iter.next() {
                            rprint!("{:02x} ", x);
                        } else {
                            rprintln!("");
                            break 'outer;
                        }
                    }
                    rprint!(" ");
                }
                rprintln!("");
            }
        }
    }
}
