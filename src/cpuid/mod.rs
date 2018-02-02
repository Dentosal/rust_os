fn run_feature_checks(_ecx: u32, _edx: u32) {
    // TODO
}

pub fn init() {
    let ecx: u32;
    let edx: u32;
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
