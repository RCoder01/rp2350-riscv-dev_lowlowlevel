use core::arch::asm;

pub struct AliasedRegister(*mut u32);

impl AliasedRegister {
    /// Base addr must be a hardware register with set/clr/xor aliases
    pub const unsafe fn new(base_addr: *mut u32) -> Self {
        Self(base_addr)
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
        unsafe { self.0.wrapping_offset(1 << 12).write_volatile(bits) }
    }

    pub fn set(self, bits: u32) {
        unsafe { self.0.wrapping_offset(2 << 12).write_volatile(bits) }
    }

    pub fn clear(self, bits: u32) {
        unsafe { self.0.wrapping_offset(3 << 12).write_volatile(bits) }
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
