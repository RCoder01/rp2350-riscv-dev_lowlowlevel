use core::ptr;

use crate::{
    common::{AliasedRegister, nop_volatile},
    extract_bits,
    resets::{RESETS_RESET, RESETS_RESET_DONE},
};

const CLOCKS_BASE: AliasedRegister = unsafe { AliasedRegister::from_addr(0x4001_0000) };
pub const CLK_REF_CTRL: AliasedRegister = unsafe { CLOCKS_BASE.offset_bytes(0x30) };
pub const CLK_REF_SELECTED: AliasedRegister = unsafe { CLOCKS_BASE.offset_bytes(0x38) };
pub const CLK_SYS_CTRL: AliasedRegister = unsafe { CLOCKS_BASE.offset_bytes(0x3C) };
pub const CLK_SYS_SELECTED: AliasedRegister = unsafe { CLOCKS_BASE.offset_bytes(0x44) };
pub const CLK_USB_CTRL: AliasedRegister = unsafe { CLOCKS_BASE.offset_bytes(0x60) };

const XOSC_BASE: AliasedRegister = unsafe { AliasedRegister::from_addr(0x4004_8000) };
pub const XOSC_CTRL: AliasedRegister = unsafe { XOSC_BASE.offset_bytes(0x0) };
pub const XOSC_STATUS: AliasedRegister = unsafe { XOSC_BASE.offset_bytes(0x4) };
pub const XOSC_STARTUP: AliasedRegister = unsafe { XOSC_BASE.offset_bytes(0xC) };
pub const XOSC_COUNT: AliasedRegister = unsafe { XOSC_BASE.offset_bytes(0x10) };

const ROSC_BASE: AliasedRegister = unsafe { AliasedRegister::from_addr(0x400E_8000) };
pub const ROSC_CTRL: AliasedRegister = unsafe { ROSC_BASE.offset_bytes(0x0) };
pub const ROSC_RANDOM: AliasedRegister = unsafe { ROSC_BASE.offset_bytes(0xC) };
pub const ROSC_RANDOMBIT: AliasedRegister = unsafe { ROSC_BASE.offset_bytes(0x20) };
pub const ROSC_COUNT: AliasedRegister = unsafe { ROSC_BASE.offset_bytes(0x24) };

const XOSC_HZ: u32 = 12_000_000;

pub fn init_xosc() {
    XOSC_STARTUP.write(0x11A);
    XOSC_CTRL.write(0xFAB_AA0);
    while (XOSC_STATUS.read() >> 31) == 0 {
        nop_volatile();
    }
}

pub const PLL_SYS_PARAMS: PllParams =
    PllParams::from_vco(1, 1_500_000_000, 5, 2).expect("Pll params should be valid");

pub fn switch_sys_clock_to_xosc() {
    assert!(CLK_REF_SELECTED.read() == 1);
    assert!(CLK_SYS_SELECTED.read() == 1);

    PLL_SYS.setup(PLL_SYS_PARAMS);

    assert_eq!(extract_bits(CLK_REF_CTRL.read(), 0..=1), 0x0);
    CLK_REF_CTRL.set(0x2);
    while extract_bits(CLK_REF_SELECTED.read(), 0..=3) != (1 << 0x2) {
        nop_volatile();
    }

    CLK_SYS_CTRL.clear(0x1);
    while extract_bits(CLK_SYS_SELECTED.read(), 0..=1) != (1 << 0x0) {
        nop_volatile();
    }
    CLK_SYS_CTRL.set((CLK_SYS_CTRL.read() ^ (0x0 << 5)) & (0b111 << 5));
    CLK_SYS_CTRL.set(0x1);
    while extract_bits(CLK_SYS_SELECTED.read(), 0..=1) != (1 << 0x1) {
        nop_volatile();
    }
}

pub const PLL_USB_PARAMS: PllParams =
    PllParams::from_vco(1, 1_200_000_000, 5, 5).expect("Pll params should be valid");

pub fn enable_usb_clock() {
    PLL_USB.setup(PLL_USB_PARAMS);

    let usb_state = CLK_USB_CTRL.read();
    assert_eq!(extract_bits(usb_state, 28..=28), 0x0);
    assert_eq!(extract_bits(usb_state, 5..=7), 0x0);
    CLK_USB_CTRL.set(1 << 11); // enable

    while CLK_USB_CTRL.read() & (1 << 28) == 0 {
        nop_volatile();
    }
}

#[derive(Copy, Clone)]
pub struct Pll {
    register: AliasedRegister,
    reset_bit: u32,
}

impl Pll {
    const fn new(register: AliasedRegister, reset_bit: u32) -> Self {
        Self {
            register,
            reset_bit,
        }
    }

    fn reset_mask(self) -> u32 {
        1 << self.reset_bit
    }

    pub fn cs(self) -> AliasedRegister {
        unsafe { self.register.offset_bytes(0x00) }
    }
    pub fn pwr(self) -> AliasedRegister {
        unsafe { self.register.offset_bytes(0x04) }
    }
    pub fn fbdiv_int(self) -> AliasedRegister {
        unsafe { self.register.offset_bytes(0x08) }
    }
    pub fn prim(self) -> AliasedRegister {
        unsafe { self.register.offset_bytes(0x0C) }
    }
    pub fn intr(self) -> AliasedRegister {
        unsafe { self.register.offset_bytes(0x10) }
    }
    pub fn inte(self) -> AliasedRegister {
        unsafe { self.register.offset_bytes(0x14) }
    }
    pub fn intf(self) -> AliasedRegister {
        unsafe { self.register.offset_bytes(0x18) }
    }
    pub fn ints(self) -> AliasedRegister {
        unsafe { self.register.offset_bytes(0x1C) }
    }

    pub fn setup(self, params: PllParams) {
        const PDIV_BITS: u32 = 7 << 16 | 7 << 12;
        let cs = self.cs().read();
        if (cs >> 31 == 1)
            && (cs & 0x3F == params.refdiv)
            && (self.fbdiv_int().read() & 0xFFF == params.fbdiv)
            && (self.prim().read() & PDIV_BITS == params.pdiv())
        {
            return;
        }
        RESETS_RESET.set(self.reset_mask());
        RESETS_RESET.clear(self.reset_mask());
        while RESETS_RESET_DONE.read() & self.reset_mask() == 0 {
            nop_volatile();
        }

        self.cs().write(params.refdiv);
        self.fbdiv_int().write(params.fbdiv);

        const PLL_PWR_PD: u32 = 1 << 0;
        const PLL_PWR_VCOPD: u32 = 1 << 5;
        self.pwr().clear(PLL_PWR_PD | PLL_PWR_VCOPD);

        while self.cs().read() >> 31 == 0 {
            nop_volatile();
        }

        self.prim().write(params.pdiv());

        const PLL_PWR_POSTDIVPD: u32 = 1 << 3;
        self.pwr().clear(PLL_PWR_POSTDIVPD);
    }
}

const RESETS_RESET_PLL_SYS: u32 = 14;
const RESETS_RESET_PLL_USB: u32 = 15;

pub const PLL_SYS: Pll = Pll::new(
    unsafe { AliasedRegister::from_addr(0x4005_0000) },
    RESETS_RESET_PLL_SYS,
);
pub const PLL_USB: Pll = Pll::new(
    unsafe { AliasedRegister::from_addr(0x4005_8000) },
    RESETS_RESET_PLL_USB,
);

#[derive(Copy, Clone)]
pub struct PllParams {
    refdiv: u32,
    fbdiv: u32,
    post_div1: u32,
    post_div2: u32,
}

impl PllParams {
    const REF_MIN: u32 = 5_000_000;
    const FBDIV_MIN: u32 = 16;
    const FBDIV_MAX: u32 = 320;
    const VCO_MIN: u32 = 750_000_000;
    const VCO_MAX: u32 = 1600_000_000;

    fn pdiv(self) -> u32 {
        self.post_div1 << 16 | self.post_div2 << 12
    }

    pub const fn new(refdiv: u32, fbdiv: u32, post_div1: u32, post_div2: u32) -> Option<Self> {
        if !(16 <= fbdiv && fbdiv <= 320) {
            return None;
        }
        let ref_freq = XOSC_HZ / refdiv;
        let vco_freq = ref_freq * fbdiv;
        let ref_max = vco_freq / 16;
        if !(Self::REF_MIN <= ref_freq && ref_freq <= ref_max) {
            return None;
        }
        if !(Self::VCO_MIN <= vco_freq && vco_freq <= Self::VCO_MAX) {
            return None;
        }
        if !(Self::FBDIV_MIN <= fbdiv && fbdiv <= Self::FBDIV_MAX) {
            return None;
        }
        if !(1 <= post_div1 && post_div1 <= 7) {
            return None;
        }
        if !(1 <= post_div2 && post_div2 <= 7) {
            return None;
        }
        Some(Self {
            refdiv,
            fbdiv,
            post_div1,
            post_div2,
        })
    }

    pub const fn from_vco(
        refdiv: u32,
        vco_freq: u32,
        post_div1: u32,
        post_div2: u32,
    ) -> Option<Self> {
        let ref_freq = XOSC_HZ / refdiv;
        let fbdiv = vco_freq / ref_freq;
        Self::new(refdiv, fbdiv, post_div1, post_div2)
    }

    pub const fn f_out_post_div_hz(self) -> u32 {
        (XOSC_HZ / self.refdiv) * self.fbdiv / (self.post_div1 * self.post_div2)
    }
}

pub fn set_ref_clock_to_rosc() {
    CLK_REF_CTRL.clear(0b1);
    while CLK_REF_SELECTED.read() != 1 {
        nop_volatile();
    }
}

pub fn set_sys_clock_to_ref() {
    CLK_SYS_CTRL.clear(0b111);
    while CLK_SYS_SELECTED.read() != 1 {
        nop_volatile();
    }
}

pub mod freq_counters {
    use crate::{
        clocks::{CLOCKS_BASE, XOSC_HZ},
        common::{AliasedRegister, nop_volatile},
    };

    pub const FC0_REF_KHZ: AliasedRegister = unsafe { CLOCKS_BASE.offset_bytes(0x8C) };
    pub const FC0_SRC: AliasedRegister = unsafe { CLOCKS_BASE.offset_bytes(0xA0) };
    pub const FC0_STATUS: AliasedRegister = unsafe { CLOCKS_BASE.offset_bytes(0xA4) };
    pub const FC0_RESULT: AliasedRegister = unsafe { CLOCKS_BASE.offset_bytes(0xA8) };

    #[repr(u32)]
    pub enum ClockToTest {
        PllSys = 0x1,
        PllUsb = 0x2,
        Rosc = 0x3,
        Xosc = 0x5,
        Ref = 0x8,
        Sys = 0x9,
        Usb = 0xB,
    }

    pub fn test_clock_khz(clock: ClockToTest) -> Result<(u32, u8), u32> {
        while FC0_STATUS.read() & (1 << 8) != 0 {
            nop_volatile();
        }
        FC0_REF_KHZ.write(XOSC_HZ / 1000);
        FC0_SRC.write(clock as u32);
        while FC0_STATUS.read() & (1 << 4) == 0 {
            nop_volatile();
        }
        let result = FC0_RESULT.read();
        let status = FC0_STATUS.read();
        FC0_SRC.write(0);
        if (status & 1) == 1 {
            let khz = result >> 5;
            let frac = (result & 0x1F) as u8;
            Ok((khz, frac))
        } else {
            Err(status)
        }
    }
}
