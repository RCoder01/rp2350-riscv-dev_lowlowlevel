use crate::common::AliasedRegister;

const RESETS_BASE: AliasedRegister = unsafe { AliasedRegister::from_addr(0x4002_0000) };
pub const RESETS_RESET: AliasedRegister = unsafe { RESETS_BASE.offset_bytes(0) };
pub const RESETS_WDSEL: AliasedRegister = unsafe { RESETS_BASE.offset_bytes(4) };
pub const RESETS_RESET_DONE: AliasedRegister = unsafe { RESETS_BASE.offset_bytes(8) };
