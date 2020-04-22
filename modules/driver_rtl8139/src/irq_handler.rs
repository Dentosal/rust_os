/// Sets interrupt handler stub on kernel.
/// The handler must read a 16-bit value from the ISR port,
/// and then write the same value back to clear all interrupts.
///
/// See https://wiki.osdev.org/RTL8139#ISR_Handler for details.
///
/// The interrupt handler stub must be a callable block of code,
/// that preserves all registers except `rax` which is used as
/// the return value.
///
/// # Safety
///
/// Please do not pass incorrect arguments, it wouldn't be nice.
#[rustfmt::skip]
pub unsafe fn set_irq_handler(irq: u8, isr_port: u16) {
    // As there is not easy way generate machine code for this,
    // it must be done the hard way: by manually writing machine code.

    // The following assembly code was assembled with nasm to produce this:
    //
    // bits 64
    // push dx
    // xor rax, rax
    // mov dx, 0x1234   ; 0x1234 as a placeholder for our port
    // in ax, dx
    // out dx, ax
    // pop dx
    // ret
    //
    // And the disassembly looks like this:
    //
    // 00000000  6652              push dx
    // 00000002  4831C0            xor rax,rax
    // 00000005  66BA3412          mov dx,0x1234
    // 00000009  66ED              in ax,dx
    // 0000000B  66EF              out dx,ax
    // 0000000D  665A              pop dx
    // 0000000F  C3                ret

    let bytes = [
        // push dx
        0x66, 0x52,
        // xor rax, rax
        0x48, 0x31, 0xc0,
        // mov dx, isr_port
        0x66, 0xba, isr_port as u8, (isr_port >> 8) as u8,
        // in ax, dx
        0x66, 0xed,
        // out dx, ax
        0x66, 0xef,
        // pop dx
        0x66, 0x5a,
        // ret
        0xc3,
    ];

    libd7::syscall::irq_set_handler(irq, &bytes).expect("Could not set IRQ handler stub");
}
