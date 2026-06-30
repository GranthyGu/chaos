// Memory management.
//
//   * helpers - p2v/v2p/k_off, verify_page_alignment, defragment_frame_pool,
//               compute_rss_watermark
//   * pgframe - PgFrame reference-counted physical page
//   * vm      - VmRegion + VmMap
//   * zone    - ZoneInfo (DMA/Normal/High zones)
//   * frame   - FramePool, frame_alloc/dealloc/contig
//   * shared  - SharedPage CoW metadata
//   * kstk    - KStk per-task kernel stack
//   * access  - user-pointer validation: check_access, cfu, ctu, validate_access
//   * heap    - heap_init, heap_grow
//   * slab    - SlabEntry fixed-size allocator
//   * buddy   - BuddyAllocator

pub mod helpers;
pub mod pgframe;
pub mod vm;
pub mod zone;
pub mod frame;
pub mod shared;
pub mod kstk;
pub mod access;
pub mod heap;
pub mod slab;
pub mod buddy;

pub use helpers::*;
pub use pgframe::*;
pub use vm::*;
pub use zone::*;
pub use frame::*;
pub use shared::*;
pub use kstk::*;
pub use access::*;
pub use heap::*;
pub use slab::*;
pub use buddy::*;
