use spin::Once;
use x86_64::instructions::segmentation::set_cs;
use x86_64::instructions::tables::{lidt, load_tss};
use x86_64::structures::gdt::SegmentSelector;
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

use self::handler::*;

#[repr(C, packed)]
struct ExceptionStackFrame {
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

        write!(f, "ExceptionStackFrame {{\n  rip: {:#x},\n  cs: {:#x},\n  flags: {:#x},\n  rsp: {:#x},\n  ss: {:#x}\n}}", ip, cs, fl, sp, ss)
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
            idt::Descriptor::new_no_ist(
                true,
                idt_table.as_u64() + table_offset,
                PrivilegeLevel::Ring0,
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
        [idt::Descriptor::new(false, 0, PrivilegeLevel::Ring0, 0); idt::ENTRY_COUNT];

    // Bind exception handlers
    handlers[0x00] = simple_exception_handler!("Divide-by-zero Error");
    handlers[0x03] = exception_handler!(exception_bp);
    handlers[0x06] = exception_handler!(exception_ud);
    handlers[0x08] = exception_handler_with_error_code!(exception_df, PrivilegeLevel::Ring0, 5);
    handlers[0x0b] = exception_handler_with_error_code!(exception_snp);
    handlers[0x0d] = exception_handler_with_error_code!(exception_gpf);
    handlers[0x0e] = exception_handler_with_error_code!(exception_pf);
    handlers[0x20] = irq_handler_switch!(exception_irq0);
    handlers[0x21] = irq_handler!(exception_irq1);
    handlers[0x21] = irq_handler!(exception_irq7);
    handlers[0x2e] = irq_handler!(exception_irq14);
    handlers[0x2e] = irq_handler!(exception_irq15);

    for index in 0..=(idt::ENTRY_COUNT - 1) {
        unsafe {
            ptr::write_volatile(
                (idt::ADDRESS + index * mem::size_of::<idt::Descriptor>()) as *mut _,
                handlers[index],
            );
        }
    }

    rprintln!("Loading new IDT...");

    unsafe {
        lidt(&DescriptorTablePointer {
            limit: (idt::ENTRY_COUNT * mem::size_of::<idt::Descriptor>()) as u16 - 1,
            base: idt::ADDRESS as u64,
        });
    }

    rprintln!("Enabled.");
}

static GDT: Once<gdt::GdtBuilder> = Once::new();
static TSS: Once<TaskStateSegment> = Once::new();

pub fn init_after_memory() {
    rprintln!("Swithcing to new GDT and TSS...");
    // Initialize TSS
    let double_fault_stack = memory::configure(|mem_ctrl: &mut MemoryController| {
        mem_ctrl
            .alloc_stack(1)
            .expect("could not allocate double fault stack")
    });

    let tss = TSS.call_once(|| {
        let mut tss = TaskStateSegment::new();
        tss.interrupt_stack_table[gdt::DOUBLE_FAULT_IST_INDEX] =
            VirtAddr::new(double_fault_stack.top.as_u64());
        tss
    });

    let mut code_selector: MaybeUninit<SegmentSelector> = MaybeUninit::uninit();
    let mut tss_selector: MaybeUninit<SegmentSelector> = MaybeUninit::uninit();

    let gdt = GDT.call_once(|| {
        let mut gdt = unsafe { gdt::GdtBuilder::new(VirtAddr::new_unchecked(0x0)) };
        code_selector.write(gdt.add_entry(gdt::Descriptor::kernel_code_segment()));
        tss_selector.write(gdt.add_entry(gdt::Descriptor::tss_segment(&tss)));
        gdt
    });

    unsafe {
        // load GDT
        gdt.load();
        // reload code segment register
        set_cs(code_selector.assume_init());
        // load TSS
        load_tss(tss_selector.assume_init());
        // Write syscall address
        ptr::write(
            (0x2000u64 as *mut u64),
            handler::process_interrupt as *const u64 as u64,
        );
    }
}

pub fn enable_external_interrupts() {
    rprintln!("Enabling external interrupts");

    unsafe {
        asm!("sti" :::: "volatile", "intel");
    }

    rprintln!("Done.");
}

pub fn disable_external_interrupts() {
    rprintln!("Enabling external interrupts");

    unsafe {
        asm!("cli" :::: "volatile", "intel");
    }

    rprintln!("Done.");
}
