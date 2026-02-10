#[unsafe(link_section = ".text._start")]
#[unsafe(naked)]
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    core::arch::naked_asm!("la sp, _stack_start", "j  main");
}
