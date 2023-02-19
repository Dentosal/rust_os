use spin::{Mutex, Once};
use x86_64::instructions::segmentation::{Segment, CS};
use x86_64::instructions::tables::{lidt, load_tss};
use x86_64::structures::gdt::SegmentSelector;
use x86_64::structures::idt::InterruptStackFrame;
use x86_64::structures::tss::TaskStateSegment;
use x86_64::structures::DescriptorTablePointer;
use x86_64::PrivilegeLevel;
use x86_64::VirtAddr;

use core::fmt;
use core::mem::{self, MaybeUninit};
use core::ptr;

use crate::memory;

#[macro_use]
mod macros;
mod gdt;
mod handler;
pub mod idt;
mod tss;

use self::handler::*;

#[derive(Clone, Copy)]
#[repr(C, packed)]
pub struct ExceptionStackFrame {
    instruction_pointer: u64,
    code_segment: u64,
    cpu_flags: u64,
    stack_pointer: u64,
    stack_segment: u64,
}
impl fmt::Display for ExceptionStackFrame {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let ip = self.instruction_pointer;
        let cs = self.code_segment;
        let fl = self.cpu_flags;
        let sp = self.stack_pointer;
        let ss = self.stack_segment;

        write!(
            f,
            concat!(
                "ExceptionStackFrame {{\n",
                "  rip: {:#x},\n",
                "  cs: {:#x},\n",
                "  flags: {:#x},\n",
                "  rsp: {:#x},\n",
                "  ss: {:#x}\n",
                "}}"
            ),
            ip, cs, fl, sp, ss
        )
    }
}
impl fmt::Debug for ExceptionStackFrame {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self)
    }
}

/// Write process descriptor tables (IDT, GDT) to given address
pub unsafe fn write_process_dts(dst: VirtAddr, idt_table: VirtAddr) {
    use x86_64::structures::gdt::DescriptorFlags as GDTF;

    let idt_desc_size = mem::size_of::<idt::Descriptor>();
    let idt_size_bytes = idt_desc_size * idt::ENTRY_COUNT;
    let idt_table_entry_size: u64 = 5; // Keep in sync with process_common .table_start

    // Write IDT
    for index in 0..idt::ENTRY_COUNT {
        let table_offset = index as u64 * idt_table_entry_size; // Jump table offset
        ptr::write(
            (dst + index * idt_desc_size).as_mut_ptr(),
            idt::Descriptor::new(
                true,
                idt_table.as_u64() + table_offset,
                PrivilegeLevel::Ring0,
                None, // TODO: set kernel stack here?
            ),
        );
    }

    // Write GDT
    let gdt_null_entry = GDTF::empty();
    let gdt_kernel_code = GDTF::USER_SEGMENT | GDTF::PRESENT | GDTF::EXECUTABLE | GDTF::LONG_MODE;
    ptr::write((dst + idt_size_bytes).as_mut_ptr(), gdt_null_entry);
    ptr::write((dst + (idt_size_bytes + 8)).as_mut_ptr(), gdt_kernel_code);
}

/// Setup kernel-mode interrupt handling
pub fn init() {
    let mut handlers: [idt::Descriptor; idt::ENTRY_COUNT] =
        [idt::Descriptor::new(false, 0, PrivilegeLevel::Ring0, None); idt::ENTRY_COUNT];

    // Bind exception handlers
    handlers[0x00] = simple_exception_handler!("Divide-by-zero Error", None);
    handlers[0x03] = exception_handler!(exception_bp);
    handlers[0x06] = exception_handler!(exception_ud);
    handlers[0x08] =
        exception_handler_with_error_code!(exception_df, PrivilegeLevel::Ring0, Some(0));
    handlers[0x0b] = exception_handler_with_error_code!(exception_snp);
    handlers[0x0d] = exception_handler_with_error_code!(exception_gpf);
    handlers[0x0e] = exception_handler_with_error_code!(exception_pf);
    handlers[0x20] = irq_handler!(exception_irq0, None);
    handlers[0x21] = irq_handler!(exception_irq1, None);
    handlers[0x27] = irq_handler!(exception_irq7, None);
    handlers[0x2e] = irq_handler!(exception_irq14, None);
    handlers[0x2f] = irq_handler!(exception_irq15, None);

    // py: print("\n".join(f"handlers[{x:#x}] = irq_handler!(irq_handler_switch, None, {x:#x});" for x in range(0x30, 0x9f+1)))
    handlers[0x30] = irq_handler_switch!(irq_dynamic, None, 0x30);
    handlers[0x31] = irq_handler_switch!(irq_dynamic, None, 0x31);
    handlers[0x32] = irq_handler_switch!(irq_dynamic, None, 0x32);
    handlers[0x33] = irq_handler_switch!(irq_dynamic, None, 0x33);
    handlers[0x34] = irq_handler_switch!(irq_dynamic, None, 0x34);
    handlers[0x35] = irq_handler_switch!(irq_dynamic, None, 0x35);
    handlers[0x36] = irq_handler_switch!(irq_dynamic, None, 0x36);
    handlers[0x37] = irq_handler_switch!(irq_dynamic, None, 0x37);
    handlers[0x38] = irq_handler_switch!(irq_dynamic, None, 0x38);
    handlers[0x39] = irq_handler_switch!(irq_dynamic, None, 0x39);
    handlers[0x3a] = irq_handler_switch!(irq_dynamic, None, 0x3a);
    handlers[0x3b] = irq_handler_switch!(irq_dynamic, None, 0x3b);
    handlers[0x3c] = irq_handler_switch!(irq_dynamic, None, 0x3c);
    handlers[0x3d] = irq_handler_switch!(irq_dynamic, None, 0x3d);
    handlers[0x3e] = irq_handler_switch!(irq_dynamic, None, 0x3e);
    handlers[0x3f] = irq_handler_switch!(irq_dynamic, None, 0x3f);
    handlers[0x40] = irq_handler_switch!(irq_dynamic, None, 0x40);
    handlers[0x41] = irq_handler_switch!(irq_dynamic, None, 0x41);
    handlers[0x42] = irq_handler_switch!(irq_dynamic, None, 0x42);
    handlers[0x43] = irq_handler_switch!(irq_dynamic, None, 0x43);
    handlers[0x44] = irq_handler_switch!(irq_dynamic, None, 0x44);
    handlers[0x45] = irq_handler_switch!(irq_dynamic, None, 0x45);
    handlers[0x46] = irq_handler_switch!(irq_dynamic, None, 0x46);
    handlers[0x47] = irq_handler_switch!(irq_dynamic, None, 0x47);
    handlers[0x48] = irq_handler_switch!(irq_dynamic, None, 0x48);
    handlers[0x49] = irq_handler_switch!(irq_dynamic, None, 0x49);
    handlers[0x4a] = irq_handler_switch!(irq_dynamic, None, 0x4a);
    handlers[0x4b] = irq_handler_switch!(irq_dynamic, None, 0x4b);
    handlers[0x4c] = irq_handler_switch!(irq_dynamic, None, 0x4c);
    handlers[0x4d] = irq_handler_switch!(irq_dynamic, None, 0x4d);
    handlers[0x4e] = irq_handler_switch!(irq_dynamic, None, 0x4e);
    handlers[0x4f] = irq_handler_switch!(irq_dynamic, None, 0x4f);
    handlers[0x50] = irq_handler_switch!(irq_dynamic, None, 0x50);
    handlers[0x51] = irq_handler_switch!(irq_dynamic, None, 0x51);
    handlers[0x52] = irq_handler_switch!(irq_dynamic, None, 0x52);
    handlers[0x53] = irq_handler_switch!(irq_dynamic, None, 0x53);
    handlers[0x54] = irq_handler_switch!(irq_dynamic, None, 0x54);
    handlers[0x55] = irq_handler_switch!(irq_dynamic, None, 0x55);
    handlers[0x56] = irq_handler_switch!(irq_dynamic, None, 0x56);
    handlers[0x57] = irq_handler_switch!(irq_dynamic, None, 0x57);
    handlers[0x58] = irq_handler_switch!(irq_dynamic, None, 0x58);
    handlers[0x59] = irq_handler_switch!(irq_dynamic, None, 0x59);
    handlers[0x5a] = irq_handler_switch!(irq_dynamic, None, 0x5a);
    handlers[0x5b] = irq_handler_switch!(irq_dynamic, None, 0x5b);
    handlers[0x5c] = irq_handler_switch!(irq_dynamic, None, 0x5c);
    handlers[0x5d] = irq_handler_switch!(irq_dynamic, None, 0x5d);
    handlers[0x5e] = irq_handler_switch!(irq_dynamic, None, 0x5e);
    handlers[0x5f] = irq_handler_switch!(irq_dynamic, None, 0x5f);
    handlers[0x60] = irq_handler_switch!(irq_dynamic, None, 0x60);
    handlers[0x61] = irq_handler_switch!(irq_dynamic, None, 0x61);
    handlers[0x62] = irq_handler_switch!(irq_dynamic, None, 0x62);
    handlers[0x63] = irq_handler_switch!(irq_dynamic, None, 0x63);
    handlers[0x64] = irq_handler_switch!(irq_dynamic, None, 0x64);
    handlers[0x65] = irq_handler_switch!(irq_dynamic, None, 0x65);
    handlers[0x66] = irq_handler_switch!(irq_dynamic, None, 0x66);
    handlers[0x67] = irq_handler_switch!(irq_dynamic, None, 0x67);
    handlers[0x68] = irq_handler_switch!(irq_dynamic, None, 0x68);
    handlers[0x69] = irq_handler_switch!(irq_dynamic, None, 0x69);
    handlers[0x6a] = irq_handler_switch!(irq_dynamic, None, 0x6a);
    handlers[0x6b] = irq_handler_switch!(irq_dynamic, None, 0x6b);
    handlers[0x6c] = irq_handler_switch!(irq_dynamic, None, 0x6c);
    handlers[0x6d] = irq_handler_switch!(irq_dynamic, None, 0x6d);
    handlers[0x6e] = irq_handler_switch!(irq_dynamic, None, 0x6e);
    handlers[0x6f] = irq_handler_switch!(irq_dynamic, None, 0x6f);
    handlers[0x70] = irq_handler_switch!(irq_dynamic, None, 0x70);
    handlers[0x71] = irq_handler_switch!(irq_dynamic, None, 0x71);
    handlers[0x72] = irq_handler_switch!(irq_dynamic, None, 0x72);
    handlers[0x73] = irq_handler_switch!(irq_dynamic, None, 0x73);
    handlers[0x74] = irq_handler_switch!(irq_dynamic, None, 0x74);
    handlers[0x75] = irq_handler_switch!(irq_dynamic, None, 0x75);
    handlers[0x76] = irq_handler_switch!(irq_dynamic, None, 0x76);
    handlers[0x77] = irq_handler_switch!(irq_dynamic, None, 0x77);
    handlers[0x78] = irq_handler_switch!(irq_dynamic, None, 0x78);
    handlers[0x79] = irq_handler_switch!(irq_dynamic, None, 0x79);
    handlers[0x7a] = irq_handler_switch!(irq_dynamic, None, 0x7a);
    handlers[0x7b] = irq_handler_switch!(irq_dynamic, None, 0x7b);
    handlers[0x7c] = irq_handler_switch!(irq_dynamic, None, 0x7c);
    handlers[0x7d] = irq_handler_switch!(irq_dynamic, None, 0x7d);
    handlers[0x7e] = irq_handler_switch!(irq_dynamic, None, 0x7e);
    handlers[0x7f] = irq_handler_switch!(irq_dynamic, None, 0x7f);
    handlers[0x80] = irq_handler_switch!(irq_dynamic, None, 0x80);
    handlers[0x81] = irq_handler_switch!(irq_dynamic, None, 0x81);
    handlers[0x82] = irq_handler_switch!(irq_dynamic, None, 0x82);
    handlers[0x83] = irq_handler_switch!(irq_dynamic, None, 0x83);
    handlers[0x84] = irq_handler_switch!(irq_dynamic, None, 0x84);
    handlers[0x85] = irq_handler_switch!(irq_dynamic, None, 0x85);
    handlers[0x86] = irq_handler_switch!(irq_dynamic, None, 0x86);
    handlers[0x87] = irq_handler_switch!(irq_dynamic, None, 0x87);
    handlers[0x88] = irq_handler_switch!(irq_dynamic, None, 0x88);
    handlers[0x89] = irq_handler_switch!(irq_dynamic, None, 0x89);
    handlers[0x8a] = irq_handler_switch!(irq_dynamic, None, 0x8a);
    handlers[0x8b] = irq_handler_switch!(irq_dynamic, None, 0x8b);
    handlers[0x8c] = irq_handler_switch!(irq_dynamic, None, 0x8c);
    handlers[0x8d] = irq_handler_switch!(irq_dynamic, None, 0x8d);
    handlers[0x8e] = irq_handler_switch!(irq_dynamic, None, 0x8e);
    handlers[0x8f] = irq_handler_switch!(irq_dynamic, None, 0x8f);
    handlers[0x90] = irq_handler_switch!(irq_dynamic, None, 0x90);
    handlers[0x91] = irq_handler_switch!(irq_dynamic, None, 0x91);
    handlers[0x92] = irq_handler_switch!(irq_dynamic, None, 0x92);
    handlers[0x93] = irq_handler_switch!(irq_dynamic, None, 0x93);
    handlers[0x94] = irq_handler_switch!(irq_dynamic, None, 0x94);
    handlers[0x95] = irq_handler_switch!(irq_dynamic, None, 0x95);
    handlers[0x96] = irq_handler_switch!(irq_dynamic, None, 0x96);
    handlers[0x97] = irq_handler_switch!(irq_dynamic, None, 0x97);
    handlers[0x98] = irq_handler_switch!(irq_dynamic, None, 0x98);
    handlers[0x99] = irq_handler_switch!(irq_dynamic, None, 0x99);
    handlers[0x9a] = irq_handler_switch!(irq_dynamic, None, 0x9a);
    handlers[0x9b] = irq_handler_switch!(irq_dynamic, None, 0x9b);
    handlers[0x9c] = irq_handler_switch!(irq_dynamic, None, 0x9c);
    handlers[0x9d] = irq_handler_switch!(irq_dynamic, None, 0x9d);
    handlers[0x9e] = irq_handler_switch!(irq_dynamic, None, 0x9e);
    handlers[0x9f] = irq_handler_switch!(irq_dynamic, None, 0x9f);

    handlers[0xd7] = simple_exception_handler!("Syscall interrupt while in kernel", None);
    handlers[0xd8] = irq_handler_switch!(exception_tsc_deadline, None, 0xd8);
    handlers[0xdd] = exception_handler!(ipi_panic);
    handlers[0xff] = simple_exception_handler!("I/O APIC masked a hardware irq", None);

    for index in 0..idt::ENTRY_COUNT {
        unsafe {
            ptr::write_volatile(
                (idt::ADDRESS + index * mem::size_of::<idt::Descriptor>()) as *mut _,
                handlers[index],
            );
        }
    }

    load_idt();
}

/// SMP AP just reuses the kernel IVT,
/// but has own GDT and TSS
pub fn init_smp_ap() {
    load_idt();
    init_gdt_and_tss();
}

fn load_idt() {
    log::debug!("Loading new IDT...");

    unsafe {
        lidt(&DescriptorTablePointer {
            limit: (idt::ENTRY_COUNT * mem::size_of::<idt::Descriptor>()) as u16 - 1,
            base: idt::VIRT_ADDRESS,
        });
    }

    log::debug!("Enabled.");
}

/// Called on BSP after the memory module (i.e. paging) has been initialized
pub fn init_after_memory() {
    init_gdt_and_tss();
    unsafe {
        // Write syscall address
        ptr::write(
            0xa000u64 as *mut u64, // TODO: constant
            handler::process_interrupt as *const u64 as u64,
        );
    }
}

fn init_gdt_and_tss() {
    // Initialize TSS
    let double_fault_stack = memory::stack_allocator::alloc_stack(1);

    let tss = tss::store({
        let mut tss = TaskStateSegment::new();
        tss.interrupt_stack_table[gdt::DOUBLE_FAULT_IST_INDEX] = double_fault_stack.top;
        tss
    });

    let kernel_cs_desc = gdt::Descriptor::kernel_code_segment();
    let tss_desc = gdt::Descriptor::tss_segment(&tss);

    let mut gdt_builder = gdt::create_new();
    let kernel_cs_sel = gdt_builder.add_entry(kernel_cs_desc);
    let tss_sel = gdt_builder.add_entry(tss_desc);

    log::debug!("Swithcing to new GDT and TSS...");
    unsafe {
        // load GDT
        gdt_builder.load();
        // reload code segment register
        CS::set_reg(kernel_cs_sel);
        // load TSS
        load_tss(tss_sel);
    }
    log::debug!("Switch done");
}
