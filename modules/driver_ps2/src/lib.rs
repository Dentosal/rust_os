#![no_std]
#![feature(allocator_api)]
#![deny(unused_must_use)]

extern crate alloc;
extern crate libd7;

use core::arch::asm;

use libd7::{ipc, select, syscall};

mod keyboard;
mod state;

use self::keyboard::Keyboard;

#[no_mangle]
fn main() -> ! {
    syscall::debug_print("PS/2 driver starting");

    // Interrupts must be disabled during initialization,
    // so this wont deadlock on not-terribly-slow computers, including Qemu
    let mut keyboard = unsafe {
        asm!("cli");
        let mut k = Keyboard::new();
        k.init();
        asm!("sti");
        k
    };

    syscall::debug_print("PS/2 keyboard initialization complete");

    // Subscribe to hardware events
    let irq = ipc::UnreliableSubscription::<u8>::exact("irq/keyboard").unwrap();

    // Inform serviced that we are running
    libd7::service::register("driver_ps2", false);

    loop {
        select! {
            one(irq) => unsafe {
                let byte = irq.receive().unwrap();
                if let Some(event) = keyboard.notify(byte) {
                    ipc::publish("keyboard/event", &event).unwrap();
                }
            }
        }
    }
}
