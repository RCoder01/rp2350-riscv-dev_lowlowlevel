use core::{arch::asm, mem::MaybeUninit, ops::Range};

use crate::assert;

#[derive(Copy, Clone)]
pub struct AliasedRegister(*mut u32);

impl AliasedRegister {
    /// Base addr must be a hardware register with set/clr/xor aliases
    pub const unsafe fn new(base_addr: *mut u32) -> Self {
        Self(base_addr)
    }

    /// Base addr must be a hardware register with set/clr/xor aliases
    pub const unsafe fn from_addr(base_addr: usize) -> Self {
        Self(core::ptr::without_provenance_mut(base_addr))
    }

    pub fn write(self, value: u32) {
        unsafe {
            self.0.write_volatile(value);
        }
    }

    pub fn read(self) -> u32 {
        unsafe { self.0.read_volatile() }
    }

    pub fn xor(self, bits: u32) {
        unsafe { self.0.wrapping_byte_offset(0x1000).write_volatile(bits) }
    }

    pub fn set(self, bits: u32) {
        unsafe { self.0.wrapping_byte_offset(0x2000).write_volatile(bits) }
    }

    pub fn clear(self, bits: u32) {
        unsafe { self.0.wrapping_byte_offset(0x3000).write_volatile(bits) }
    }

    /// addr+4*words must be a hardware register with set/clr/xor aliases
    pub const unsafe fn offset(self, words: usize) -> Self {
        Self(self.0.wrapping_add(words))
    }

    /// addr+bytes must be a hardware register with set/clr/xor aliases
    pub const unsafe fn offset_bytes(self, bytes: usize) -> Self {
        Self(self.0.wrapping_byte_add(bytes))
    }
}

pub struct Defer<F: FnOnce()> {
    f: MaybeUninit<F>,
}

impl<F: FnOnce()> Defer<F> {
    pub fn new(f: F) -> Self {
        Self {
            f: MaybeUninit::new(f),
        }
    }
}

impl<F: FnOnce()> Drop for Defer<F> {
    fn drop(&mut self) {
        let mut new_f = MaybeUninit::uninit();
        core::mem::swap(&mut self.f, &mut new_f);
        let f = unsafe { new_f.assume_init() };
        f();
    }
}

#[inline(always)]
pub fn nop_volatile() {
    unsafe {
        asm!("", options(nomem, nostack, preserves_flags));
    }
}

pub unsafe fn csr_read<const CSR: u32>() -> usize {
    let read: usize;
    unsafe {
        asm!("csrrsi    {read}, {CSR}, 0", read = out(reg) read, CSR = const CSR);
    }
    read
}

pub unsafe fn csr_write<const CSR: u32>(value: usize) {
    unsafe {
        asm!("csrw     {CSR}, {value}", value = in(reg) value, CSR = const CSR);
    }
}

pub unsafe fn csr_read_write<const CSR: u32>(value: usize) -> usize {
    let read: usize;
    unsafe {
        asm!("csrrw     {read}, {CSR}, {value}", read = out(reg) read, value = in(reg) value, CSR = const CSR);
    }
    read
}

pub unsafe fn csr_read_write_imm<const CSR: u32, const VALUE: usize>() -> usize {
    let read: usize;
    unsafe {
        asm!("csrrwi     {read}, {CSR}, {value}", read = out(reg) read, value = const VALUE, CSR = const CSR);
    }
    read
}

pub unsafe fn csr_set<const CSR: u32>(value: usize) {
    unsafe {
        asm!("csrs     {CSR}, {value}", value = in(reg) value, CSR = const CSR);
    }
}

pub unsafe fn csr_set_imm<const CSR: u32, const VALUE: usize>() {
    unsafe {
        asm!("csrsi    {CSR}, {value}", value = const VALUE, CSR = const CSR);
    }
}

pub unsafe fn csr_read_set<const CSR: u32>(value: usize) -> usize {
    let read: usize;
    unsafe {
        asm!("csrrs     {read}, {CSR}, {value}", read = out(reg) read, value = in(reg) value, CSR = const CSR);
    }
    read
}

pub unsafe fn csr_read_set_imm<const CSR: u32, const VALUE: usize>() -> usize {
    let read: usize;
    unsafe {
        asm!("csrrsi     {read}, {CSR}, {value}", read = out(reg) read, value = const VALUE, CSR = const CSR);
    }
    read
}

pub unsafe fn csr_clear<const CSR: u32>(value: usize) {
    unsafe {
        asm!("csrc     {CSR}, {value}", value = in(reg) value, CSR = const CSR);
    }
}

pub unsafe fn csr_clear_imm<const CSR: u32, const VALUE: usize>() {
    unsafe {
        asm!("csrci    {CSR}, {value}", value = const VALUE, CSR = const CSR);
    }
}

pub unsafe fn csr_read_clear<const CSR: u32>(value: usize) -> usize {
    let read: usize;
    unsafe {
        asm!("csrrc     {read}, {CSR}, {value}", read = out(reg) read, value = in(reg) value, CSR = const CSR);
    }
    read
}

pub unsafe fn csr_read_clear_imm<const CSR: u32, const VALUE: usize>() -> usize {
    let read: usize;
    unsafe {
        asm!("csrrc     {read}, {CSR}, {value}", read = out(reg) read, value = const VALUE, CSR = const CSR);
    }
    read
}

pub const fn copy_const(dst: &mut [u8], range: Range<usize>, src: &[u8]) {
    assert!(range.end >= range.start);
    let len = range.end - range.start;
    assert!(dst.len() >= range.end);
    assert!(src.len() == len);
    let mut i = 0;
    while i < len {
        dst[range.start + i] = src[i];
        i += 1;
    }
}

#[macro_export]
macro_rules! const_for {
    ($val:ident in $arr:expr => $expr:block) => {
        let mut i = 0;
        let arr = $arr;
        while i < arr.len() {
            let $val = &arr[i];
            $expr
            i += 1;
        }
    };
}
