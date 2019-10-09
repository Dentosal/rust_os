use core::hint::unreachable_unchecked;

#[allow(non_camel_case_types)]
#[repr(u64)]
pub enum SysCallNumber {
    exit = 0x00,
}


pub fn exit(return_code: u64) -> ! {
    unsafe {
        asm!("int 0xd7" :: "{rax}"(SysCallNumber::exit), "{rdi}"(return_code) :: "intel");
        unreachable_unchecked();
    }
}
