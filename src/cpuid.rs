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
        const TSCD         = 1 << 24;
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
            asm!(
                "xor ecx, ecx; xor edx, edx; cpuid",
                inout("ecx") 0 => ecx,
                inout("edx") 0 => edx,
                out("eax") eax,
                out("ebx") ebx,
                options(nostack)
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

/// Returns tuple (max_standard_level, max_extended_level)
fn get_max_levels() -> (u32, u32) {
    let max_standard_level: u32;
    unsafe {
        asm!("cpuid",
            inout("eax") 0 => max_standard_level,
            out("rbx") _,
            out("rdx") _,
            out("rcx") _,
            options(nostack, nomem)
        );
    }
    let max_extended_level: u32;
    unsafe {
        asm!("cpuid",
            inout("eax") 0x8000_0000u32 => max_extended_level,
            out("rbx") _,
            out("rdx") _,
            out("rcx") _,
            options(nostack, nomem)
        );
    }
    (max_standard_level, max_extended_level)
}

macro_rules! assert_feature {
    ($register:expr, $feature:expr) => {
        assert!(
            $register.contains($feature),
            "Unsupported CPU: Feature {:?} missing",
            $feature
        );
    };
}

fn run_feature_checks() {
    let (level_std, level_ext) = get_max_levels();
    assert!(
        level_std >= 3,
        "CPUID standard max level too low {:x} < 3",
        level_std
    );
    assert!(
        level_ext >= 0x8000_0007,
        "CPUID extended max level too low {:x} < 0x8000_0007",
        level_ext
    );

    let ecx: u32;
    let edx: u32;
    unsafe {
        // CPUID_GETFEATURES
        asm!(
            "cpuid",
            inout("eax") 1 => _,
            out("ebx") _,
            out("ecx") ecx,
            out("edx") edx,
            options(nostack, nomem)
        );
    }
    log::debug!("CPU: FEATURE BITS: {:b} {:b}", ecx, edx);

    let f_ecx = FlagsECX::from_bits_truncate(ecx);
    let f_edx = FlagsEDX::from_bits_truncate(edx);

    assert_feature!(f_edx, FlagsEDX::TSC);
    assert_feature!(f_ecx, FlagsECX::TSCD);
    assert_feature!(f_edx, FlagsEDX::SSE);
    assert_feature!(f_edx, FlagsEDX::APIC);

    let edx: u32;
    unsafe {
        // Get extended capabilities
        asm!(
            "xor ecx, ecx; xor edx, edx; mov eax, 1; cpuid",
            inout("eax") 0x8000_0007u32 => _,
            out("ebx") _,
            out("ecx") _,
            out("edx") edx,
            options(nostack, nomem)
        );
    }
    assert!(edx & (1 << 8) != 0, "CPUID: invariant TSC not supported");
}

pub fn init() {
    log::debug!("CPU: {}", cpu_brand());
    run_feature_checks();
}
