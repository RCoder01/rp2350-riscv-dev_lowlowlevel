use core::arch::naked_asm;

use crate::{
    blink_partial_value, blink_value,
    common::{Defer, csr_clear_imm, csr_read, csr_read_set_imm, csr_set, csr_set_imm, csr_write},
    delay,
    gpio::{LED_PIN, gpio_output_xor},
    timer_trap_handler,
    usb::usb_trap_handler,
};

const RVCSR_MSTATUS: u32 = 0x300;
const RVCSR_MIE: u32 = 0x304;
const RVCSR_MTVEC: u32 = 0x305;
const RVCSR_MEPC: u32 = 0x341;
const RVCSR_MCAUSE: u32 = 0x342;
const RVCSR_MEINEXT: u32 = 0xBE4;
const RVCSR_MEICONTEXT: u32 = 0xBE5;

pub const RVCSR_MEIFA: u32 = 0xBE2;
pub const RVCSR_MEIEA: u32 = 0xBE0;

const RVCSR_MSTATUS_MIE: usize = 1 << 3;
const RVCSR_MIE_MEIE: usize = 1 << 11;
pub fn init_traps() {
    let handler = trap_handler_wrapper as extern "C" fn() as usize;

    unsafe { csr_write::<RVCSR_MTVEC>(handler) };
    unsafe { csr_set_imm::<RVCSR_MSTATUS, RVCSR_MSTATUS_MIE>() };
    unsafe { csr_set::<RVCSR_MIE>(RVCSR_MIE_MEIE) };
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
    Defer::new(|| unsafe { csr_write::<RVCSR_MCAUSE>(cause) });

    let addr = unsafe { csr_read::<RVCSR_MEPC>() };
    Defer::new(|| unsafe { csr_write::<RVCSR_MEPC>(addr) });

    match cause {
        0x0 => {
            // instruction alignment, never fires
            unreachable!()
        }
        0x1 => {
            // instruction access fault
        }
        0x2 => {
            // illegal instruction
        }
        0x3 => {
            // breakpoint
        }
        0x4 => {
            // load align
        }
        0x5 => {
            // load fault
        }
        0x6 => {
            // store align
        }
        0x7 => {
            // store fault
        }
        0x8 => {
            // ecall from U-mode
        }
        0xB => {
            // ecall from M-mode
        }
        ..=0x7FFF_FFFF => {
            // unknown interrupt
            blink_trap_cause(cause, addr)
        }
        0x8000_0003 => {
            // soft irq
        }
        0x8000_0007 => {
            // timer irq
        }
        0x8000_000B => {
            // external irq
            return handle_external_interrupt();
        }
        0x8000_0000.. => {
            // unknown interrupt
            blink_trap_cause(cause, addr)
        }
    }
    blink_trap_cause_once(cause, addr);
}

fn handle_external_interrupt() {
    const RVCSR_MEICONTEXT_CLEARTS: usize = 1 << 1;
    let meicontext = unsafe { csr_read_set_imm::<RVCSR_MEICONTEXT, RVCSR_MEICONTEXT_CLEARTS>() };
    Defer::new(|| unsafe { csr_write::<RVCSR_MEICONTEXT>(meicontext) });

    loop {
        const RVCSR_MEINEXT_UPDATE: usize = 1 << 0;
        let next = unsafe { csr_read_set_imm::<RVCSR_MEINEXT, RVCSR_MEINEXT_UPDATE>() };
        if next >> 31 != 0 {
            break;
        }
        let next_irq = next >> 2;
        let curr_context = unsafe { csr_read::<RVCSR_MEICONTEXT>() };
        assert_eq!(curr_context >> 15 & 0b1, next >> 31);
        assert_eq!(curr_context >> 4 & 0xFF, next_irq & 0xFF);
        assert!(unsafe { csr_read::<0x344>() & (1 << 11) } != 0);

        unsafe { csr_set_imm::<RVCSR_MSTATUS, RVCSR_MSTATUS_MIE>() };
        // Defer::new(|| unsafe { csr_clear_imm::<RVCSR_MSTATUS, RVCSR_MSTATUS_MIE>() });

        // blink_partial_value(unsafe { csr_read::<0xBE1>() } >> 16, 16);

        // table 95 in rp2350 datasheet
        match next_irq {
            0..=7 => {
                // timer interrupts
                timer_trap_handler(next_irq);
            }
            14 => {
                usb_trap_handler();
            }
            ..=51 => loop {
                fast_blink(40);
                blink_partial_value(next, 6);
            },
            52.. => {
                unreachable!()
            }
        };
        unsafe { csr_clear_imm::<RVCSR_MSTATUS, RVCSR_MSTATUS_MIE>() };
    }
}

fn blink_trap_cause(cause: usize, addr: usize) -> ! {
    loop {
        blink_trap_cause_once(cause, addr);
    }
}

fn blink_trap_cause_once(cause: usize, addr: usize) {
    fast_blink(20);
    blink_partial_value(cause >> 31, 1);
    fast_blink(5);
    blink_partial_value(cause, 8);
    fast_blink(5);
    blink_value(addr);
}

fn fast_blink(count: usize) {
    for _ in 0..2 * count {
        gpio_output_xor(LED_PIN);
        delay(1);
    }
}
