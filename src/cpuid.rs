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

pub fn init() {
    let ebx: u32;
    let ecx: u32;
    let edx: u32;
    unsafe {
        // CPUID_GETFEATURES
        asm!(
            "xor ecx, ecx; xor edx, edx; mov eax, 1; cpuid"
            : "={ecx}"(ebx), "={ecx}"(ecx), "={edx}"(edx)
            :
            : "eax"
            : "intel", "volatile"
        );
    }
    log::debug!("CPU: {}", cpu_brand());
    log::debug!("CPU: FEATURE BITS: {:b} {:b} {:b}", ebx, ecx, edx);
}
