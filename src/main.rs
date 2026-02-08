#![no_std]
#![no_main]

const A: *mut u32 = 0x4002_80cc_usize as _;
const B: *mut u32 = 0x4003_8068_usize as _;
const C: *mut u32 = 0xd000_0038_usize as _;
const XOR: *mut u32 = 0xD000_0028_usize as _;

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    unsafe {
        A.write_volatile(5);
        B.write_volatile(0x34);
        C.write_volatile(1u32 << 25);
    }
    'outer: loop {
        unsafe { XOR.write_volatile(1u32 << 25) };
        let mut r = 1u32 << 25;
        loop {
            r -= 1 << 3;
            unsafe {
                core::arch::asm!("", options(nomem, nostack, preserves_flags));
            }
            if r == 0 {
                continue 'outer;
            }
        }
    }
}

#[panic_handler]
fn panic_handler(_: &core::panic::PanicInfo) -> ! {
    loop {}
}
