pub mod ticks {
    use crate::common::AliasedRegister;

    const TICKS_BASE: AliasedRegister = unsafe { AliasedRegister::from_addr(0x4010_8000) };
    pub const PROC0: TickGenerator = unsafe { TickGenerator::new(TICKS_BASE.offset_bytes(0x00)) };
    pub const PROC1: TickGenerator = unsafe { TickGenerator::new(TICKS_BASE.offset_bytes(0x0C)) };
    pub const TIMER0: TickGenerator = unsafe { TickGenerator::new(TICKS_BASE.offset_bytes(0x18)) };
    pub const TIMER1: TickGenerator = unsafe { TickGenerator::new(TICKS_BASE.offset_bytes(0x24)) };
    pub const WATCHDOG: TickGenerator =
        unsafe { TickGenerator::new(TICKS_BASE.offset_bytes(0x30)) };
    pub const RISCV: TickGenerator = unsafe { TickGenerator::new(TICKS_BASE.offset_bytes(0x3C)) };

    #[derive(Copy, Clone)]
    pub struct TickGenerator {
        base: AliasedRegister,
    }

    impl TickGenerator {
        /// Requires that the passed register is the start of a 3-register (ctrl, cycles, count) set
        const unsafe fn new(base: AliasedRegister) -> Self {
            Self { base }
        }

        fn ctrl(self) -> AliasedRegister {
            unsafe { self.base.offset_bytes(0x0) }
        }

        fn cycles(self) -> AliasedRegister {
            unsafe { self.base.offset_bytes(0x4) }
        }

        fn count(self) -> AliasedRegister {
            unsafe { self.base.offset_bytes(0x8) }
        }

        pub fn enable(self, clk_ref_mhz: u32) {
            const MHZ_MASK: u32 = 0x1FF;
            assert!(clk_ref_mhz & !MHZ_MASK == 0);

            const ENABLE: u32 = 1 << 0;
            self.ctrl().clear(ENABLE);
            self.cycles().write(clk_ref_mhz & MHZ_MASK);
            self.ctrl().set(ENABLE);
        }

        pub fn read(self) -> u32 {
            self.count().read()
        }
    }
}

pub mod timer {
    use core::time::Duration;

    use crate::{
        common::{AliasedRegister, csr_clear, csr_set, nop_volatile},
        resets::{RESETS_RESET, RESETS_RESET_DONE},
        trap::{RVCSR_MEIEA, RVCSR_MEIFA},
    };

    #[derive(Copy, Clone)]
    pub struct Timer {
        base: AliasedRegister,
        first_irq_num: u32,
        reset_mask: u32,
    }

    impl Timer {
        /// Requires that the passed register is the start of a timer register series
        const unsafe fn new(base: AliasedRegister, first_irq_num: u32, reset_mask: u32) -> Self {
            Self {
                base,
                first_irq_num,
                reset_mask,
            }
        }

        fn high_write(self) -> AliasedRegister {
            unsafe { self.base.offset_bytes(0x00) }
        }

        fn low_write(self) -> AliasedRegister {
            unsafe { self.base.offset_bytes(0x04) }
        }

        fn high_read(self) -> AliasedRegister {
            unsafe { self.base.offset_bytes(0x08) }
        }

        fn low_read(self) -> AliasedRegister {
            unsafe { self.base.offset_bytes(0x0C) }
        }

        fn alarm_0(self) -> AliasedRegister {
            unsafe { self.base.offset_bytes(0x10) }
        }

        fn alarm_1(self) -> AliasedRegister {
            unsafe { self.base.offset_bytes(0x14) }
        }

        fn alarm_2(self) -> AliasedRegister {
            unsafe { self.base.offset_bytes(0x18) }
        }

        fn alarm_3(self) -> AliasedRegister {
            unsafe { self.base.offset_bytes(0x1C) }
        }

        pub fn armed(self) -> AliasedRegister {
            unsafe { self.base.offset_bytes(0x20) }
        }

        fn raw_high(self) -> AliasedRegister {
            unsafe { self.base.offset_bytes(0x24) }
        }

        fn raw_low(self) -> AliasedRegister {
            unsafe { self.base.offset_bytes(0x28) }
        }

        fn debug_pause(self) -> AliasedRegister {
            unsafe { self.base.offset_bytes(0x2C) }
        }

        fn pause(self) -> AliasedRegister {
            unsafe { self.base.offset_bytes(0x30) }
        }

        fn locked(self) -> AliasedRegister {
            unsafe { self.base.offset_bytes(0x34) }
        }

        fn source(self) -> AliasedRegister {
            unsafe { self.base.offset_bytes(0x38) }
        }

        fn intr(self) -> AliasedRegister {
            unsafe { self.base.offset_bytes(0x3C) }
        }

        fn inte(self) -> AliasedRegister {
            unsafe { self.base.offset_bytes(0x40) }
        }

        fn intf(self) -> AliasedRegister {
            unsafe { self.base.offset_bytes(0x44) }
        }

        fn ints(self) -> AliasedRegister {
            unsafe { self.base.offset_bytes(0x48) }
        }

        // not thread safe
        unsafe fn write_time(self, val: u64) {
            let low = (val & 0xFFFF_FFFF) as u32;
            let high = (val >> 32) as u32;
            self.low_write().write(low);
            self.high_write().write(high);
        }

        fn alarm(self, alarm: Alarm) -> AliasedRegister {
            match alarm {
                Alarm::Alarm0 => self.alarm_0(),
                Alarm::Alarm1 => self.alarm_1(),
                Alarm::Alarm2 => self.alarm_2(),
                Alarm::Alarm3 => self.alarm_3(),
            }
        }

        pub fn reset(self) {
            RESETS_RESET.set(self.reset_mask);
            RESETS_RESET.clear(self.reset_mask);
            while RESETS_RESET_DONE.read() & self.reset_mask == 0 {
                nop_volatile();
            }
        }

        // not thread safe
        pub unsafe fn read_time(self) -> u64 {
            let low = self.low_read().read();
            let high = self.high_read().read();
            ((high as u64) << 32) | (low as u64)
        }

        fn enable_alarm_interrupts(self) {
            assert!(self.first_irq_num.is_multiple_of(4));
            // lower 4 bits specify which 16-bit window, upper 16 bits mask the window
            let window = (self.first_irq_num / 16) as usize;
            let mask = 0b1110 << (self.first_irq_num % 16);
            let select = (mask << 16) + window;
            unsafe { csr_clear::<RVCSR_MEIFA>(select) }; // remove any pending forced interrupts
            unsafe { csr_set::<RVCSR_MEIEA>(select) }; // enable timer interrupts
        }

        pub fn enable_alarms(self) {
            self.enable_alarm_interrupts();
            self.intf().clear(0b1111);
            self.inte().set(0b1111);
        }

        pub fn set_alarm(self, alarm: Alarm, duration: Duration) -> Result<(), TooFarInTheFuture> {
            let micros = duration.as_micros();
            if micros as u32 as u128 != micros {
                return Err(TooFarInTheFuture);
            }
            let target_diff = micros as u32;
            let now_lower = self.raw_low().read();
            let target = now_lower.wrapping_add(target_diff);
            self.intf().clear(1 << (alarm as u32));
            self.alarm(alarm).write(target);
            Ok(())
        }
    }

    #[derive(Copy, Clone, Debug)]
    pub struct TooFarInTheFuture;

    #[derive(Copy, Clone)]
    pub enum Alarm {
        Alarm0 = 0b00,
        Alarm1 = 0b01,
        Alarm2 = 0b10,
        Alarm3 = 0b11,
    }

    const TIMER0_BASE: AliasedRegister = unsafe { AliasedRegister::from_addr(0x400B_0000) };
    const TIMER1_BASE: AliasedRegister = unsafe { AliasedRegister::from_addr(0x400B_0000) };
    pub const TIMER0: Timer = unsafe { Timer::new(TIMER0_BASE, 0, 1 << 23) };
    pub const TIMER1: Timer = unsafe { Timer::new(TIMER1_BASE, 4, 1 << 24) };
}
