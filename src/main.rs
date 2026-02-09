#![no_std]
#![no_main]

#[repr(u8)]
#[allow(unused)]
#[derive(Clone, Copy)]
enum GpioPin {
    GPIO0 = 0,
    GPIO1 = 1,
    GPIO2 = 2,
    GPIO3 = 3,
    GPIO4 = 4,
    GPIO5 = 5,
    GPIO6 = 6,
    GPIO7 = 7,
    GPIO8 = 8,
    GPIO9 = 9,
    GPIO10 = 10,
    GPIO11 = 11,
    GPIO12 = 12,
    GPIO13 = 13,
    GPIO14 = 14,
    GPIO15 = 15,
    GPIO16 = 16,
    GPIO17 = 17,
    GPIO18 = 18,
    GPIO19 = 19,
    GPIO20 = 20,
    GPIO21 = 21,
    GPIO22 = 22,
    GPIO23 = 23,
    GPIO24 = 24,
    GPIO25 = 25,
    GPIO26 = 26,
    GPIO27 = 27,
    GPIO28 = 28,
    GPIO29 = 29,
    GPIO30 = 30,
    GPIO31 = 31,
    GPIO32 = 32,
    GPIO33 = 33,
    GPIO34 = 34,
    GPIO35 = 35,
    GPIO36 = 36,
    GPIO37 = 37,
    GPIO38 = 38,
    GPIO39 = 39,
    GPIO40 = 40,
    GPIO41 = 41,
    GPIO42 = 42,
    GPIO43 = 43,
    GPIO44 = 44,
    GPIO45 = 45,
    GPIO46 = 46,
    GPIO47 = 47,
}

impl GpioPin {
    const fn from_u8(pin: u8) -> Option<Self> {
        match pin {
            0 => Some(Self::GPIO0),
            1 => Some(Self::GPIO1),
            2 => Some(Self::GPIO2),
            3 => Some(Self::GPIO3),
            4 => Some(Self::GPIO4),
            5 => Some(Self::GPIO5),
            6 => Some(Self::GPIO6),
            7 => Some(Self::GPIO7),
            8 => Some(Self::GPIO8),
            9 => Some(Self::GPIO9),
            10 => Some(Self::GPIO10),
            11 => Some(Self::GPIO11),
            12 => Some(Self::GPIO12),
            13 => Some(Self::GPIO13),
            14 => Some(Self::GPIO14),
            15 => Some(Self::GPIO15),
            16 => Some(Self::GPIO16),
            17 => Some(Self::GPIO17),
            18 => Some(Self::GPIO18),
            19 => Some(Self::GPIO19),
            20 => Some(Self::GPIO20),
            21 => Some(Self::GPIO21),
            22 => Some(Self::GPIO22),
            23 => Some(Self::GPIO23),
            24 => Some(Self::GPIO24),
            25 => Some(Self::GPIO25),
            26 => Some(Self::GPIO26),
            27 => Some(Self::GPIO27),
            28 => Some(Self::GPIO28),
            29 => Some(Self::GPIO29),
            30 => Some(Self::GPIO30),
            31 => Some(Self::GPIO31),
            32 => Some(Self::GPIO32),
            33 => Some(Self::GPIO33),
            34 => Some(Self::GPIO34),
            35 => Some(Self::GPIO35),
            36 => Some(Self::GPIO36),
            37 => Some(Self::GPIO37),
            38 => Some(Self::GPIO38),
            39 => Some(Self::GPIO39),
            40 => Some(Self::GPIO40),
            41 => Some(Self::GPIO41),
            42 => Some(Self::GPIO42),
            43 => Some(Self::GPIO43),
            44 => Some(Self::GPIO44),
            45 => Some(Self::GPIO45),
            46 => Some(Self::GPIO46),
            47 => Some(Self::GPIO47),
            _ => None,
        }
    }
}

const IO_BANK0_BASE: *mut u32 = 0x4002_8000 as _;
const PADS_BANK0_BASE: *mut u32 = 0x4003_8000 as _;

const fn gpio_status_reg(pin: GpioPin) -> *mut u32 {
    unsafe { IO_BANK0_BASE.offset((pin as u8 * 2) as _) }
}

const fn gpio_ctrl_reg(pin: GpioPin) -> *mut u32 {
    unsafe { IO_BANK0_BASE.offset((pin as u8 * 2 + 1) as _) }
}

const fn pads_gpio_reg(pin: GpioPin) -> *mut u32 {
    unsafe { PADS_BANK0_BASE.offset((pin as u8 + 1) as _) }
}

const SIO_GPIO_OE_SET: *mut u32 = 0xd000_0038_usize as _;
const SIO_GPIO_OUT_XOR: *mut u32 = 0xD000_0028_usize as _;

fn gpio_output_enable(pin: GpioPin) {
    if (pin as u8) < 32 {
        unsafe { SIO_GPIO_OE_SET.write_volatile(1 << (pin as u32)) }
    } else {
        unsafe { SIO_GPIO_OE_SET.offset(1).write_volatile(1 << (pin as u32)) }
    }
}

fn gpio_output_xor(pin: GpioPin) {
    if (pin as u8) < 32 {
        unsafe { SIO_GPIO_OUT_XOR.write_volatile(1 << (pin as u32)) }
    } else {
        unsafe { SIO_GPIO_OUT_XOR.offset(1).write_volatile(1 << (pin as u32)) }
    }
}

const LED_PIN: GpioPin = GpioPin::GPIO25;

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    const SIO: u32 = 5;

    const DRIVE_STRENGTH_12MA: u32 = 0x3 << 4;
    const PULL_DOWN_ENABLE: u32 = 1 << 2;

    unsafe {
        gpio_ctrl_reg(LED_PIN).write_volatile(SIO);
        pads_gpio_reg(LED_PIN).write_volatile(DRIVE_STRENGTH_12MA | PULL_DOWN_ENABLE);
        gpio_output_enable(LED_PIN);
    }

    'outer: loop {
        gpio_output_xor(LED_PIN);
        let mut r = 1u32 << 20;
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
