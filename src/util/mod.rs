pub mod elf_parser;

use cpuio::Port;

macro_rules! raw_ptr {
    (u8 $ptr:expr ; $offset:expr) => (
        *(($ptr as *const u8).offset($offset / 1))
    );
    (u16 $ptr:expr ; $offset:expr) => (
        *(($ptr as *const u16).offset($offset / 2))
    );
    (u32 $ptr:expr ; $offset:expr) => (
        *(($ptr as *const u32).offset($offset / 4))
    );
    (u64 $ptr:expr ; $offset:expr) => (
        *(($ptr as *const u64).offset($offset / 8))
    );

    // default to index 0
    (u8 $ptr:expr) => (
        raw_ptr!(u8 $ptr; 0x0)
    );
    (u16 $ptr:expr) => (
        raw_ptr!(u16 $ptr; 0x0)
    );
    (u32 $ptr:expr) => (
        raw_ptr!(u32 $ptr; 0x0)
    );
    (u64 $ptr:expr) => (
        raw_ptr!(u64 $ptr; 0x0)
    );

    // default to u8
    ($ptr:expr, $offset:expr) => (
        raw_ptr!(u8 $ptr; $offset)
    );

    // default u8 at index 0
    ($ptr:expr) => (
        raw_ptr!(u8 $ptr; 0x0)
    );
}

macro_rules! dump_memory_at {
    ($ptr:expr) => (rprintln!("{:x} {:x} {:x} {:x}", raw_ptr!(u16 $ptr; 0), raw_ptr!(u16 $ptr; 2), raw_ptr!(u16 $ptr; 4), raw_ptr!(u16 $ptr; 6)));
}

macro_rules! int {
    ($num:expr) => ({
        asm!(concat!("int ", stringify!($num)) :::: "volatile", "intel");
    });
}

macro_rules! bochs_magic_bp {
    () => ({
        #![allow(unused_unsafe)]
        unsafe {
            asm!("xchg bx, bx" :::: "volatile", "intel");
        };
    });
}

macro_rules! no_interrupts {
    ($block:expr) => {
        unsafe {
            asm!("cli" :::: "volatile", "intel");
        }
        $block;
        unsafe {
            asm!("sti" :::: "volatile", "intel");
        }
    }
}

macro_rules! sizeof {
    ($t:ty) => {{ ::core::mem::size_of::<$t>() }};
}

pub fn io_wait() {
    unsafe {
        let mut io_wait_port: Port<u8> = Port::new(0x80);
        io_wait_port.write(0);
    }
}

pub unsafe fn inb(port: u16) -> u8 {
    let mut port: Port<u8> = Port::new(port);
    port.read()
}

pub unsafe fn outb(port: u16, data: u8) {
    let mut port: Port<u8> = Port::new(port);
    port.write(data)
}
