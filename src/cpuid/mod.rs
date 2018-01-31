fn run_feature_checks(ecx: u32, edx: u32) {

}

pub fn init() {
    let mut ecx: u32;
    let mut edx: u32;
    unsafe {
        // CPUID_GETFEATURES
        asm!(
            "xor ecx, ecx; xor edx, edx; mov eax, 1; cpuid"
            : "={ecx}"(ecx), "={edx}"(edx) :: "eax", "ecx", "edx"
            : "intel", "volatile"
        );
    }
    rprintln!("CPU FEATURE BITS: {:b} {:b}", ecx, edx);
    run_feature_checks(ecx, edx);
}
