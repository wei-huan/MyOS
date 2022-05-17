pub const PAGE_SIZE: usize = 4096;
pub const PAGE_SIZE_BITS: usize = 0xc;
pub const MEMORY_END: usize = 0x80800000;
// 内核堆大小2MB
pub const KERNEL_HEAP_SIZE: usize = 0x20_0000;
// 给buddy_system_allocator使用的，这个值大于32即可
pub const HEAP_ALLOCATOR_MAX_ORDER: usize = 32;
pub const KERNEL_STACK_SIZE: usize = PAGE_SIZE * 2;
pub const TRAP_CONTEXT: usize = TRAMPOLINE - PAGE_SIZE;
pub const USER_STACK_SIZE: usize = PAGE_SIZE * 2;
pub const USER_STACK_HIGH: usize = TRAP_CONTEXT - PAGE_SIZE; // 再减一个 PAGE_SIZE 为Guard Page
pub const TRAMPOLINE: usize = usize::MAX - PAGE_SIZE + 1;
pub const MMAP_BASE: usize = 0x60000000;
// 启动栈大小64KB
pub const BOOT_STACK_SIZE: usize = PAGE_SIZE * 16;
