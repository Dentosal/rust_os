use alloc::borrow::ToOwned;
use alloc::string::String;

bitflags::bitflags! {
    pub struct FlagsECX: u32 {
        const SSE3         = 1 << 0;
        const PCLMUL       = 1 << 1;
        const DTES64       = 1 << 2;
        const MONITOR      = 1 << 3;
        const DS_CPL       = 1 << 4;
        const VMX          = 1 << 5;
        const SMX          = 1 << 6;
        const EST          = 1 << 7;
        const TM2          = 1 << 8;
        const SSSE3        = 1 << 9;
        const CID          = 1 << 10;
        const FMA          = 1 << 12;
        const CX16         = 1 << 13;
        const ETPRD        = 1 << 14;
        const PDCM         = 1 << 15;
        const PCIDE        = 1 << 17;
        const DCA          = 1 << 18;
        const SSE4_1       = 1 << 19;
        const SSE4_2       = 1 << 20;
        const X2APIC       = 1 << 21;
        const MOVBE        = 1 << 22;
        const POPCNT       = 1 << 23;
        const AES          = 1 << 25;
        const XSAVE        = 1 << 26;
        const OSXSAVE      = 1 << 27;
        const AVX          = 1 << 28;
    }
}

bitflags::bitflags! {
    pub struct FlagsEDX: u32 {
        const FPU          = 1 << 0;
        const VME          = 1 << 1;
        const DE           = 1 << 2;
        const PSE          = 1 << 3;
        const TSC          = 1 << 4;
        const MSR          = 1 << 5;
        const PAE          = 1 << 6;
        const MCE          = 1 << 7;
        const CX8          = 1 << 8;
        const APIC         = 1 << 9;
        const SEP          = 1 << 11;
        const MTRR         = 1 << 12;
        const PGE          = 1 << 13;
        const MCA          = 1 << 14;
        const CMOV         = 1 << 15;
        const PAT          = 1 << 16;
        const PSE36        = 1 << 17;
        const PSN          = 1 << 18;
        const CLF          = 1 << 19;
        const DTES         = 1 << 21;
        const ACPI         = 1 << 22;
        const MMX          = 1 << 23;
        const FXSR         = 1 << 24;
        const SSE          = 1 << 25;
        const SSE2         = 1 << 26;
        const SS           = 1 << 27;
        const HTT          = 1 << 28;
        const TM1          = 1 << 29;
        const IA64         = 1 << 30;
        const PBE          = 1 << 31;
    }
}

pub fn cpu_brand() -> String {
    let mut result = String::new();

    'outer: for index in 0x80000002u32..=0x80000004u32 {
        let eax: u32;
        let ebx: u32;
        let ecx: u32;
        let edx: u32;
        unsafe {
            llvm_asm!(
                "xor ecx, ecx; xor edx, edx; cpuid"
                :
                    "={eax}"(eax), "={ebx}"(ebx),
                    "={ecx}"(ecx), "={edx}"(edx)
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
        llvm_asm!(
            "xor ecx, ecx; xor edx, edx; mov eax, 1; cpuid"
            : "={ecx}"(ecx), "={edx}"(edx)
            :
            : "eax", "ebx"
            : "intel", "volatile"
        );
    }
    log::debug!("CPU: {}", cpu_brand());
    log::debug!("CPU: FEATURE BITS: {:b} {:b}", ecx, edx);
    run_feature_checks(ecx, edx);
}
