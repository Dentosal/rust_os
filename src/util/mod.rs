pub unsafe fn cr2() -> u64 {
    let value: u64;
    asm!("mov rax, cr2" : "={rax}"(value) ::: "volatile", "intel");
    value
}

macro_rules! int {
    ($num:expr) => ({
        asm!(concat!("int ", stringify!($num)) :::: "intel");
    });
}
