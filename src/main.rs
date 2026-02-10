#![no_std]
#![no_main]

use crate::{
    common::nop_volatile,
    gpio::{
        DRIVE_STRENGTH_12MA, LED_PIN, PULL_DOWN_ENABLE, SIO, gpio_ctrl_reg, gpio_output_clear,
        gpio_output_enable, gpio_output_set, gpio_output_xor, pads_gpio_reg,
    },
    trap::init_traps,
    usb::init_usb_blocking,
};

mod common;
mod gpio;
mod startup;
mod trap;
mod usb;

#[unsafe(no_mangle)]
pub extern "C" fn main() -> ! {
    unsafe {
        gpio_ctrl_reg(LED_PIN).write_volatile(SIO);
        pads_gpio_reg(LED_PIN).write_volatile(DRIVE_STRENGTH_12MA | PULL_DOWN_ENABLE);
        gpio_output_enable(LED_PIN);
    }
    init_traps();
    // init_usb_blocking();

    const RANDOM_RAM: *mut u32 = 0x2000_0004 as _;
    unsafe { RANDOM_RAM.write_volatile(0b1001_0110) };
    let val = unsafe { RANDOM_RAM.read_volatile() };

    blink_partial_value(val as _, 4);

    unsafe { (0x2000_0005 as *const u32).read_volatile() };

    loop {}
}

fn blink_value(val: usize) {
    blink_partial_value(val, 32)
}

fn blink_partial_value(mut val: usize, num_bits: u32) {
    // lsb first
    for _ in 0..num_bits {
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
    gpio_output_clear(LED_PIN);
    delay(12);
    for _ in 0..2 {
        delay(4);
        gpio_output_xor(LED_PIN);
    }
    delay(8);
}

fn blink_1() {
    gpio_output_clear(LED_PIN);
    delay(12);
    for _ in 0..4 {
        delay(4);
        gpio_output_xor(LED_PIN);
    }
}

fn delay(units: u32) {
    let count = units * 3 << 16;
    for _ in 0..count {
        nop_volatile();
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
            nop_volatile();
            if r == 0 {
                continue 'outer;
            }
        }
    }
}
