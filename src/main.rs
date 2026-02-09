#![no_std]
#![no_main]

use core::arch::asm;

use crate::gpio::{
    DRIVE_STRENGTH_12MA, LED_PIN, PULL_DOWN_ENABLE, SIO, gpio_ctrl_reg, gpio_output_clear,
    gpio_output_enable, gpio_output_set, gpio_output_xor, pads_gpio_reg,
};

mod gpio;
mod startup;

#[unsafe(no_mangle)]
pub extern "C" fn main() -> ! {
    unsafe {
        gpio_ctrl_reg(LED_PIN).write_volatile(SIO);
        pads_gpio_reg(LED_PIN).write_volatile(DRIVE_STRENGTH_12MA | PULL_DOWN_ENABLE);
        gpio_output_enable(LED_PIN);
    }

    const RANDOM_RAM: *mut u32 = 0x2000_0004 as _;
    unsafe { RANDOM_RAM.write_volatile(0b1001_0110) };
    let val = unsafe { RANDOM_RAM.read_volatile() };

    blink_value(val as _);

    panic!();
}

fn blink_value(mut val: usize) {
    // lsb first
    for _ in 0..32 {
        let bit = val & 1;
        if bit == 1 {
            blink_1();
        } else {
            blink_0();
        }
        val >>= 1;
    }
}

fn blink_0() {
    delay(16);
    gpio_output_set(LED_PIN);
    delay(4);
    gpio_output_clear(LED_PIN);
}

fn blink_1() {
    delay(16);
    gpio_output_set(LED_PIN);
    delay(4);
    gpio_output_clear(LED_PIN);
    delay(4);
    gpio_output_set(LED_PIN);
    delay(4);
    gpio_output_clear(LED_PIN);
}

fn delay(units: u32) {
    let count = units << 18;
    for _ in 0..count {
        unsafe {
            asm!("", options(nomem, nostack, preserves_flags));
        }
    }
}

#[panic_handler]
fn panic_handler(_: &core::panic::PanicInfo) -> ! {
    unsafe {
        gpio_ctrl_reg(LED_PIN).write_volatile(SIO);
        pads_gpio_reg(LED_PIN).write_volatile(DRIVE_STRENGTH_12MA | PULL_DOWN_ENABLE);
        gpio_output_enable(LED_PIN);
    }

    const PERIOD: usize = 1 << 18;
    'outer: loop {
        gpio_output_xor(LED_PIN);
        let mut r = PERIOD;
        loop {
            r -= 1;
            unsafe {
                asm!("", options(nomem, nostack, preserves_flags));
            }
            if r == 0 {
                continue 'outer;
            }
        }
    }
}
