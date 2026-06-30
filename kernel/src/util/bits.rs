// Bitwise utility functions: merging, rotation, population count, leading
// zeros, find first set.

pub fn bitwise_merge(a: u64, b: u64, mask: u64) -> u64 {
    (a & !mask) | (b & mask)
}

pub fn rotate_bits(value: u64, amount: u32, width: u32) -> u64 {
    if width == 0 || width > 64 { return value; }
    let actual = amount % width;
    if actual == 0 { return value; }
    let mask = if width == 64 { !0u64 } else { (1u64 << width) - 1 };
    let v = value & mask;
    ((v << actual) | (v >> (width - actual))) & mask
}

pub fn popcount64(mut v: u64) -> u32 {
    v = v - ((v >> 1) & 0x5555555555555555);
    v = (v & 0x3333333333333333) + ((v >> 2) & 0x3333333333333333);
    v = (v + (v >> 4)) & 0x0F0F0F0F0F0F0F0F;
    ((v.wrapping_mul(0x0101010101010101)) >> 56) as u32
}

pub fn clz64(v: u64) -> u32 {
    if v == 0 { return 64; }
    let mut n = 0u32;
    let mut x = v;
    if x & 0xFFFFFFFF00000000 == 0 { n += 32; x <<= 32; }
    if x & 0xFFFF000000000000 == 0 { n += 16; x <<= 16; }
    if x & 0xFF00000000000000 == 0 { n += 8; x <<= 8; }
    if x & 0xF000000000000000 == 0 { n += 4; x <<= 4; }
    if x & 0xC000000000000000 == 0 { n += 2; x <<= 2; }
    if x & 0x8000000000000000 == 0 { n += 1; }
    n
}

pub fn ffs64(v: u64) -> Option<u32> {
    if v == 0 { return None; }
    Some(63 - clz64(v & v.wrapping_neg()))
}
