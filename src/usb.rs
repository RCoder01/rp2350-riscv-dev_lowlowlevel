use core::ptr;

use crate::common::{AliasedRegister, csr_clear, csr_set, nop_volatile};

pub const RESETS_BASE: *mut u32 = ptr::without_provenance_mut(0x4002_0000);
pub const RESETS_RESET: AliasedRegister =
    unsafe { AliasedRegister::new(RESETS_BASE.wrapping_offset(0)) };
pub const RESETS_WDSEL: AliasedRegister =
    unsafe { AliasedRegister::new(RESETS_BASE.wrapping_offset(1)) };
pub const RESETS_RESET_DONE: AliasedRegister =
    unsafe { AliasedRegister::new(RESETS_BASE.wrapping_offset(2)) };

const RESETS_RESET_USBCTRL: u32 = 28;
const USBCTRL_MASK: u32 = 1 << RESETS_RESET_USBCTRL;

const USBCTRL_DPRAM_BASE: *mut u8 = ptr::without_provenance_mut(0x50100000);
const USBCTRL_DPRAM_LEN: usize = 4 * 1024;
const USBCTRL_DPRAM: *mut [u8; USBCTRL_DPRAM_LEN] = USBCTRL_DPRAM_BASE.cast();

fn reset_usb() {
    RESETS_RESET.set(USBCTRL_MASK);
    RESETS_RESET.clear(USBCTRL_MASK);
    while (!RESETS_RESET_DONE.read()) & USBCTRL_MASK != 0 {
        nop_volatile();
    }
}

const RVCSR_MEIFA: u32 = 0xBE2;
const RVCSR_MEIEA: u32 = 0xBE2;
const USBCTRL_IRQ: u32 = 25;

fn enable_usbctrl_interrupt() {
    // lower 4 bits specify which 16-bit window, upper 16 bits mask the window
    let usbctrl = (1 << (USBCTRL_IRQ % 16)) << 16 + (USBCTRL_IRQ / 16);
    unsafe { csr_clear::<RVCSR_MEIFA>(usbctrl) }; // remove any pending forced interrupts
    unsafe { csr_set::<RVCSR_MEIEA>(usbctrl) }; // enable usbctrl interrupt
}

pub fn init_usb_blocking() {
    reset_usb();
    unsafe {
        USBCTRL_DPRAM.write_volatile([0; _]);
    }
    enable_usbctrl_interrupt();
}
