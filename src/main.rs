#![no_std]
#![no_main]
#![forbid(unsafe_op_in_unsafe_fn)]

use core::{ops::RangeInclusive, time::Duration};

use crate::{
    clocks::{enable_usb_clock, init_xosc, switch_sys_clock_to_xosc},
    common::nop_volatile,
    gpio::{
        DRIVE_STRENGTH_12MA, LED_PIN, PULL_DOWN_ENABLE, SIO, gpio_ctrl_reg, gpio_output_clear,
        gpio_output_enable, gpio_output_xor, pads_gpio_reg,
    },
    timer::timer::{Alarm, TIMER0},
    trap::init_traps,
    usb::init_usb_as_device,
};

mod clocks;
mod common;
mod gpio;
mod resets;
mod startup;
mod timer;
mod trap;
mod usb;

#[macro_export]
macro_rules! assert_eq {
    ($a: expr, $b: expr, $msg: expr) => {
        if ($a) != ($b) {
            panic!($msg);
        }
    };
    ($a: expr, $b: expr) => {
        assert_eq!($a, $b, "");
    };
}

#[macro_export]
macro_rules! assert {
    ($val: expr, $msg: expr) => {
        if !($val) {
            panic!($msg);
        }
    };
    ($val: expr) => {
        assert!($val, "");
    };
}

pub extern "C" fn main() -> ! {
    unsafe {
        gpio_ctrl_reg(LED_PIN).write_volatile(SIO);
        pads_gpio_reg(LED_PIN).write_volatile(DRIVE_STRENGTH_12MA | PULL_DOWN_ENABLE);
        gpio_output_enable(LED_PIN);
    }

    init_xosc();
    switch_sys_clock_to_xosc();
    enable_usb_clock();

    init_traps();
    init_alarms();
    enable_timer();
    init_usb_as_device();

    loop {
        gpio_output_xor(LED_PIN);
        delay(10);
    }
}

fn init_alarms() {
    timer::ticks::TIMER0.enable(clocks::XOSC_HZ / 1_000_000);
    TIMER0.reset();
    TIMER0.enable_alarms();
}

fn enable_timer() {
    TIMER0
        .set_alarm(Alarm::Alarm1, Duration::from_millis(1_000))
        .expect("Duration is not too long");
}

pub fn timer_trap_handler(_alarm: usize) {
    assert!(_alarm == 1);
    TIMER0.clear_alarm_event(Alarm::Alarm1);
    // gpio_output_xor(LED_PIN);
    enable_timer();
}

fn extract_bits(word: u32, range: RangeInclusive<u32>) -> u32 {
    assert!(*range.end() <= 31);
    assert!(range.end() >= range.start());
    if *range.end() == 31 {
        word >> range.start()
    } else {
        (word & ((1 << (range.end() + 1)) - 1)) >> range.start()
    }
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
    let count = (units * 3) << 20;
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

    const PERIOD: usize = 1 << 21;
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
