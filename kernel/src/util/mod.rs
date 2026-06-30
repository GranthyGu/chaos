// Utility submodule.
//
// Pure helper functions with no kernel-state dependencies:
//   * bits     - bitwise tricks (popcount, clz, ffs, rotate, merge)
//   * align    - power-of-two alignment helpers
//   * hash     - hash combination, murmur3 finalizer, crc32
//   * varint   - LEB128-style variable-length integer codec
//   * memscan  - KMP pattern match

pub mod bits;
pub mod align;
pub mod hash;
pub mod varint;
pub mod memscan;

pub use bits::*;
pub use align::*;
pub use hash::*;
pub use varint::*;
pub use memscan::*;
