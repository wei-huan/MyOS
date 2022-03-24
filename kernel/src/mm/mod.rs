mod address;
mod address_space;
mod frame;
mod heap;
mod page_table;
mod section;

pub use address::{addr_test, PhysAddr, PhysPageNum, StepByOne, VirtAddr, VirtPageNum};
pub use address_space::{kernel_token, remap_test, AddrSpace, KERNEL_SPACE};
pub use frame::{frame_alloc, frame_allocator_test, frame_dealloc, frame_test, Frame};
pub use heap::{heap_test, init_heap, whereis_heap};
pub use page_table::{
    translated_byte_buffer, translated_str, PageTable, PageTableEntry, UserBuffer,
};
pub use section::Permission;

use crate::cpu::cpu_id;

pub fn boot_init() {
    heap::init_heap();
    frame::init_frame_allocator();
    KERNEL_SPACE[cpu_id()].exclusive_access().activate();
}

pub fn init() {
    KERNEL_SPACE[cpu_id()].exclusive_access().activate();
}
