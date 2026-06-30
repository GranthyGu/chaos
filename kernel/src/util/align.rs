// Power-of-two alignment helpers.

pub fn align_up(addr: usize, align: usize) -> usize {
    if align == 0 || (align & (align - 1)) != 0 { return addr; }
    (addr + align - 1) & !(align - 1)
}

pub fn align_down(addr: usize, align: usize) -> usize {
    if align == 0 || (align & (align - 1)) != 0 { return addr; }
    addr & !(align - 1)
}

pub fn is_power_of_two(v: usize) -> bool {
    v != 0 && (v & (v - 1)) == 0
}

pub fn log2_floor(v: usize) -> usize {
    if v == 0 { return 0; }
    (std::mem::size_of::<usize>() * 8) - 1 - (v.leading_zeros() as usize)
}
