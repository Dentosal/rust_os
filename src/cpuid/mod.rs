use alloc::borrow::ToOwned;
use alloc::string::String;

pub fn cpu_brand() -> String {
    let mut result = String::new();

    'outer: for index in 0x80000002u32..=0x80000004u32 {
        let eax: u32;
        let ebx: u32;
        let ecx: u32;
        let edx: u32;
        unsafe {
            asm!(
                "xor ecx, ecx; xor edx, edx; cpuid"
                : "={eax}"(eax), "={ebx}"(ebx), "={ecx}"(ecx), "={edx}"(edx)
                : "{eax}"(index)
                :
                : "intel", "volatile"
            );
            let values = [eax, ebx, ecx, edx];
            for v in values.iter() {
                for i in 0..=3 {
                    let bytepos = i * 8;
                    let byte = ((v & (0xFF << bytepos)) >> bytepos) as u8;
                    if byte == 0 {
                        break 'outer;
                    }
                    result.push(byte as char);
                }
            }
        }
    }
    result.trim().to_owned()
}

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
            : "={ecx}"(ecx), "={edx}"(edx)
            :
            : "eax", "ebx"
            : "intel", "volatile"
        );
    }
    rprintln!("CPU: {}", cpu_brand());
    rprintln!("CPU: FEATURE BITS: {:b} {:b}", ecx, edx);
    run_feature_checks(ecx, edx);
}
