#![no_std]
#![no_main]

const GPIO25_CTRL_REG: *mut u32 = 0x4002_80cc_usize as _;
const PADS_BANK0_GPIO25: *mut u32 = 0x4003_8068_usize as _;
const SIO_GPIO_OE_SET: *mut u32 = 0xd000_0038_usize as _;
const SIO_GPIO_OUT_XOR: *mut u32 = 0xD000_0028_usize as _;

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    const SIO: u32 = 5;

    const DRIVE_STRENGTH_12MA: u32 = 0x3 << 4;
    const PULL_DOWN_ENABLE: u32 = 1 << 2;
    unsafe {
        GPIO25_CTRL_REG.write_volatile(SIO);
        PADS_BANK0_GPIO25.write_volatile(DRIVE_STRENGTH_12MA | PULL_DOWN_ENABLE);
        SIO_GPIO_OE_SET.write_volatile(1 << 25);
    }
    'outer: loop {
        unsafe { SIO_GPIO_OUT_XOR.write_volatile(1 << 25) };
        let mut r = 1u32 << 22;
        loop {
            r -= 1;
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
