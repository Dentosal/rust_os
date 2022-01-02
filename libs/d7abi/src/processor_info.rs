use core::arch::asm;

/// An array of these is available for all processes, index of the array is processor id.
/// They allow retrieving static information about processor cores
/// without having to do system calls. A process can get it's cpu id,
/// i.e. index in the processor core list, using `rdtscp`.
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct ProcessorInfo {
    /// TSC frequency in Hz, assumed to be invariant
    pub tsc_freq_hz: u64,
    /// TSC values on different CPUs can be compared
    /// using `tsc_offset + read_tsc()`
    pub tsc_offset: u64,
}

/// Reads entry for any processor.
/// The cpu_id can be retrieved using `rdtscp`, and whn doing TSC-related
/// arithmetic, gets the right entry for the returned timestamp.
///
/// # Safety
///
/// Must be only called in when process page tables are active,
/// kernel mode doesn't have anything mapped to this address.
///
/// `cpu_id` must be valid.
pub unsafe fn read(cpu_id: u32) -> &'static ProcessorInfo {
    let ptr: *const ProcessorInfo = crate::kernel_constants::PROCESS_PROCESSOR_INFO_TABLE.as_ptr();
    &*ptr.add(cpu_id as usize)
}

/// Reads entry for the current processor.
/// Note that process switch can occur anywhere in usermode,
/// and the data returned by the entry might not for the current cpu.
///
/// # Safety
///
/// Must be only called in when process page tables are active,
/// kernel mode doesn't have anything mapped to this address.
pub unsafe fn read_current() -> &'static ProcessorInfo {
    let rcx: u64;
    asm!(
        "rdtscp",
        out("rdx") _,
        out("rax") _,
        out("rcx") rcx,
        options(nomem, nostack)
    );
    read(rcx as u32)
}
