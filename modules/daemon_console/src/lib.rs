//! Console driver.
//! Manages VGA buffer and the keyboard.
//!
//! Has normal tty-consoles in 1-9 and kerenl log in 0.
//! The active console can be switched with `ctrl-alt-number`.
//!
//! TODO: color support

#![no_std]
#![feature(ptr_internals)]
#![deny(unused_must_use)]

#[macro_use]
extern crate alloc;

#[macro_use]
extern crate libd7;

use alloc::borrow::ToOwned;
use alloc::string::String;
use alloc::vec::Vec;
use hashbrown::HashSet;

use libd7::{
    ipc::{self, protocol::keyboard::KeyboardEvent, InternalSubscription, SubscriptionId},
    process::ProcessId,
    select, syscall,
};

mod keyboard;
mod vga;
mod virtual_console;

use self::keyboard::Keyboard;
use self::virtual_console::VirtualConsole;

struct Console {
    device: VirtualConsole,
    sub_print: ipc::ReliableSubscription<String>,
}
impl Console {
    pub fn new(name: &str) -> Self {
        Self {
            device: VirtualConsole::new(),
            sub_print: ipc::ReliableSubscription::exact(&format!("console/{}", name)).unwrap(),
        }
    }

    pub fn receive_print(&mut self) {
        let (ack_ctx, message) = self.sub_print.receive().unwrap();
        self.device.output.write_str(message.as_bytes());
        ack_ctx.ack().unwrap();
    }
}

#[no_mangle]
fn main() -> ! {
    println!("Console daemon starting");

    let mut active_index: usize = 0; // Kernel log active by default
    let mut consoles = vec![
        Console::new("kernel_log"),
        Console::new("1"),
        Console::new("2"),
        Console::new("3"),
        Console::new("4"),
        Console::new("5"),
        Console::new("6"),
        Console::new("7"),
        Console::new("8"),
        Console::new("9"),
    ];

    let mut keyboard = Keyboard::new();
    let mut vga_buffer = unsafe { vga::get_hardware_buffer() };

    consoles[0].device.render(&mut vga_buffer);

    let kbd_sub = ipc::UnreliableSubscription::<KeyboardEvent>::exact("keyboard/event").unwrap();
    let c_sub_ids: Vec<SubscriptionId> = consoles.iter().map(|c| c.sub_print.sub_id()).collect();

    // Inform the serviced that we are up
    libd7::service::register("consoled", false);

    loop {
        select! {
            any(c_sub_ids) -> c_index => {
                let console = consoles.get_mut(c_index).unwrap();
                console.receive_print();
                if c_index == active_index {
                    console.device.render(&mut vga_buffer);
                }
            },
            one(kbd_sub) => {
                let event = kbd_sub.receive().unwrap();
                let action = keyboard.process_event(event);

                let mut mods_ctrl = HashSet::new();
                mods_ctrl.insert(d7keymap::KeySymbol::new("LeftCtrl"));
                if let self::keyboard::EventAction::Unmatched(k, mods) = &action {
                    if mods == &mods_ctrl {
                        if let Ok(number) = k.as_str().parse::<usize>() {
                            active_index = number;
                        }
                    }
                }

                if active_index != 0 {
                    consoles[active_index].device.input.keyboard_event(action);
                }

                consoles[active_index].device.render(&mut vga_buffer);
            }
        }
    }
}
