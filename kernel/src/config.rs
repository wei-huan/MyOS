pub const PAGE_SIZE: usize = 4096;
pub const PAGE_SIZE_BITS: usize = 0xc;
pub const MEMORY_END: usize = 0x82800000;
pub const KERNEL_HEAP_SIZE: usize = 0x30_0000;

// for buddy_system_allocator
pub const HEAP_ALLOCATOR_MAX_ORDER: usize = 32;

// Kernel and User Address Space
pub const TRAMPOLINE: usize = usize::MAX - PAGE_SIZE + 1;

// Kernel Address Space
pub const KERNEL_STACK_BASE: usize = TRAMPOLINE - 2 * PAGE_SIZE; // stack grow down, so stack base address is high end
pub const KERNEL_STACK_SIZE: usize = PAGE_SIZE * 2;
pub const BOOT_STACK_SIZE: usize = PAGE_SIZE * 4; // 16 KB

// User Address Space
pub const TRAP_CONTEXT_BASE: usize = TRAMPOLINE - PAGE_SIZE;
pub const USER_STACK_BASE: usize = 0xFFFFFFC000000000; // stack grow down, so stack base address is high end
pub const USER_STACK_SIZE: usize = PAGE_SIZE * 8;
pub const MMAP_BASE: usize = 0x10_0000_0000; // 0xFFFFFFC000000000;
pub const DLL_LOADER_BASE: usize = 0x30_0000_0000; // dynamic link library loader base address

#[inline(always)]
pub fn page_aligned_up(addr: usize) -> usize {
    (addr + PAGE_SIZE - 1) / PAGE_SIZE * PAGE_SIZE
}

#[inline(always)]
pub fn is_page_aligned(addr: usize) -> bool {
    addr % PAGE_SIZE == 0
}
