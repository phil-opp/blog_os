pub fn flush() {
    unsafe{asm!("mov rax, cr3
        mov cr3, rax" ::: "{rax}" : "intel")}
}
