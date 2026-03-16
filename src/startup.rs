use crate::main;

#[unsafe(link_section = ".text._start")]
#[unsafe(naked)]
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    core::arch::naked_asm!(
        "la sp, _stack_start",
        "j  {}",
        ".p2align 3",
        ".word 0xFFFFDED3",
        ".word 0x11010142",
        ".word 0x000001FF",
        ".word 0x00000000",
        ".word 0xAB123579",
        sym main,
    );
}
