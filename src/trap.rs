use core::arch::{asm, naked_asm};

use crate::{
    blink_partial_value, blink_value,
    common::{csr_read, csr_read_set_imm, csr_read_write},
    delay,
    gpio::{LED_PIN, gpio_output_xor},
};

const RVCSR_MSTATUS: u32 = 0x300;
const RVCSR_MTVEC: u32 = 0x305;
const RVCSR_MEPC: u32 = 0x341;
const RVCSR_MCAUSE: u32 = 0x342;

pub fn init_traps() {
    let handler = trap_handler_wrapper as extern "C" fn() as usize;
    let _ = unsafe { csr_read_write::<RVCSR_MTVEC>(handler) };

    const RVCSR_MSTATUS_MIE: u32 = 3;
    const RVCSR_MSTATUS_MIE_MASK: usize = (1 << RVCSR_MSTATUS_MIE) as _;
    unsafe { csr_read_set_imm::<RVCSR_MSTATUS, RVCSR_MSTATUS_MIE_MASK>() };
}

#[unsafe(naked)]
extern "C" fn trap_handler_wrapper() {
    naked_asm!(
        "addi   sp, sp, -18*4",
        "sw     x1, -0*4(sp)",
        "sw     x3, -1*4(sp)",
        "sw     x4, -2*4(sp)",
        "sw     x5, -3*4(sp)",
        "sw     x6, -4*4(sp)",
        "sw     x7, -5*4(sp)",
        "sw     x10, -6*4(sp)",
        "sw     x11, -7*4(sp)",
        "sw     x12, -8*4(sp)",
        "sw     x13, -9*4(sp)",
        "sw     x14, -10*4(sp)",
        "sw     x15, -11*4(sp)",
        "sw     x16, -12*4(sp)",
        "sw     x17, -13*4(sp)",
        "sw     x28, -14*4(sp)",
        "sw     x29, -15*4(sp)",
        "sw     x30, -16*4(sp)",
        "sw     x31, -17*4(sp)",
        // trap handler will save callee-saved registers if necessary
        "call   trap_handler",
        "lw     x1, -0*4(sp)",
        "lw     x3, -1*4(sp)",
        "lw     x4, -2*4(sp)",
        "lw     x5, -3*4(sp)",
        "lw     x6, -4*4(sp)",
        "lw     x7, -5*4(sp)",
        "lw     x10, -6*4(sp)",
        "lw     x11, -7*4(sp)",
        "lw     x12, -8*4(sp)",
        "lw     x13, -9*4(sp)",
        "lw     x14, -10*4(sp)",
        "lw     x15, -11*4(sp)",
        "lw     x16, -12*4(sp)",
        "lw     x17, -13*4(sp)",
        "lw     x28, -14*4(sp)",
        "lw     x29, -15*4(sp)",
        "lw     x30, -16*4(sp)",
        "lw     x31, -17*4(sp)",
        "addi   sp, sp, 18*4",
        "mret"
    );
}

#[unsafe(no_mangle)]
extern "C" fn trap_handler() {
    let cause = unsafe { csr_read::<RVCSR_MCAUSE>() };
    let addr = unsafe { csr_read::<RVCSR_MEPC>() };
    if cause >> 31 == 0 {
        blink_trap_cause(cause, addr);
    }
}

fn blink_trap_cause(cause: usize, addr: usize) -> ! {
    loop {
        fast_blink(20);
        blink_partial_value(cause >> 31, 1);
        fast_blink(5);
        blink_partial_value(cause, 8);
        fast_blink(5);
        blink_value(addr);
    }
}

fn fast_blink(count: usize) {
    for _ in 0..2 * count {
        gpio_output_xor(LED_PIN);
        delay(1);
    }
}
