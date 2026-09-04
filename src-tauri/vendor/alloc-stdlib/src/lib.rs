#![cfg_attr(not(feature = "unsafe"), forbid(unsafe_code))]

#[macro_use]
extern crate alloc_no_stdlib;

pub mod heap_alloc;
pub mod std_alloc;

pub use self::heap_alloc::{HeapAlloc, HeapPrealloc};
#[cfg(feature = "unsafe")]
pub use self::heap_alloc::HeapAllocUninitialized;
pub use self::std_alloc::StandardAlloc;

pub use alloc_no_stdlib::{AllocatedStackMemory, Allocator, SliceWrapper, SliceWrapperMut, StackAllocator};
