use spin::{Mutex, Once};
use x86_64::instructions::segmentation::set_cs;
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

use crate::memory::{self, MemoryController};

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
    let call_instruction_size: u64 = 5; // Call instruction size in bytes

    // Write IDT
    for index in 0..idt::ENTRY_COUNT {
        let table_offset = index as u64 * call_instruction_size; // Jump table offset
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
    handlers[0x29] = irq_handler!(exception_irq9, None);
    handlers[0x2a] = irq_handler!(exception_irq10, None);
    handlers[0x2b] = irq_handler!(exception_irq11, None);
    handlers[0x2e] = irq_handler!(exception_irq14, None);
    handlers[0x2f] = irq_handler!(exception_irq15, None);
    handlers[0x30] = irq_handler_switch!(exception_tsc_deadline, None);
    handlers[0xdd] = exception_handler!(ipi_panic);

    for index in 0..idt::ENTRY_COUNT {
        log::trace!(
            "WRITE IDT {:x} @ {:04x}",
            index,
            idt::ADDRESS + index * mem::size_of::<idt::Descriptor>()
        );
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
    let double_fault_stack = memory::configure(|mem_ctrl: &mut MemoryController| {
        mem_ctrl
            .alloc_stack(1)
            .expect("could not allocate double fault stack")
    });

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
        set_cs(kernel_cs_sel);
        // load TSS
        load_tss(tss_sel);
    }
    log::debug!("Switch done");
}
