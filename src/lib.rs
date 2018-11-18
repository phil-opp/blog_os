#![cfg_attr(not(test), no_std)] // don't link the Rust standard library
#![feature(abi_x86_interrupt)]

#[macro_use]
pub mod vga_buffer;
pub mod gdt;
pub mod interrupts;
pub mod serial;

pub unsafe fn exit_qemu() {
    use x86_64::instructions::port::Port;

    let mut port = Port::<u32>::new(0xf4);
    port.write(0);
}

pub fn hlt_loop() -> ! {
    loop {
        x86_64::instructions::hlt();
    }
}
