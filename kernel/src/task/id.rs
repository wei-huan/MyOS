use crate::config::{
    KERNEL_STACK_BASE, KERNEL_STACK_SIZE, PAGE_SIZE, TRAP_CONTEXT_BASE, USER_STACK_BASE,
    USER_STACK_SIZE,
};
use crate::mm::{MapPermission, VirtAddr, KERNEL_SPACE};
use alloc::string::ToString;
use alloc::vec::Vec;
use lazy_static::*;
use spin::Mutex;

pub struct RecycleAllocator {
    current: usize,
    recycled: Vec<usize>,
}

impl RecycleAllocator {
    pub fn new() -> Self {
        RecycleAllocator {
            current: 0,
            recycled: Vec::new(),
        }
    }
    pub fn alloc(&mut self) -> usize {
        if let Some(id) = self.recycled.pop() {
            id
        } else {
            self.current += 1;
            self.current - 1
        }
    }
    pub fn dealloc(&mut self, id: usize) {
        assert!(id < self.current);
        assert!(
            !self.recycled.iter().any(|i| *i == id),
            "id {} has been deallocated!",
            id
        );
        self.recycled.push(id);
    }
}

lazy_static! {
    static ref PID_ALLOCATOR: Mutex<RecycleAllocator> = Mutex::new(RecycleAllocator::new());
    static ref TID_ALLOCATOR: Mutex<RecycleAllocator> = Mutex::new(RecycleAllocator::new());    // Like Linux, each thread in os has unique id
    static ref KSTACK_ALLOCATOR: Mutex<RecycleAllocator> = Mutex::new(RecycleAllocator::new());
}

pub struct PidHandle(pub usize);

pub fn pid_alloc() -> PidHandle {
    PidHandle(PID_ALLOCATOR.lock().alloc())
}

impl Drop for PidHandle {
    fn drop(&mut self) {
        PID_ALLOCATOR.lock().dealloc(self.0);
    }
}

pub struct TidHandle(pub usize);

pub fn tid_alloc() -> TidHandle {
    TidHandle(TID_ALLOCATOR.lock().alloc())
}

impl Drop for TidHandle {
    fn drop(&mut self) {
        TID_ALLOCATOR.lock().dealloc(self.0);
    }
}

/// Return (bottom, top) of a kernel stack in kernel space.
pub fn kernel_stack_position(kstack_id: usize) -> (usize, usize) {
    let top = KERNEL_STACK_BASE - kstack_id * (KERNEL_STACK_SIZE + PAGE_SIZE);
    let bottom = top - KERNEL_STACK_SIZE;
    (bottom, top)
}

pub struct KernelStack(pub usize);

pub fn kstack_alloc() -> KernelStack {
    let kstack_id = KSTACK_ALLOCATOR.lock().alloc();
    let (kstack_bottom, kstack_top) = kernel_stack_position(kstack_id);
    KERNEL_SPACE.lock().insert_framed_area(
        ".kstack".to_string(),
        kstack_bottom.into(),
        kstack_top.into(),
        MapPermission::R | MapPermission::W,
    );
    KernelStack(kstack_id)
}

impl Drop for KernelStack {
    fn drop(&mut self) {
        let (kernel_stack_bottom, _) = kernel_stack_position(self.0);
        let kernel_stack_bottom_va: VirtAddr = kernel_stack_bottom.into();
        KERNEL_SPACE
            .lock()
            .remove_area_with_start_vpn(kernel_stack_bottom_va.into());
    }
}

impl KernelStack {
    #[allow(unused)]
    pub fn push_on_top<T>(&self, value: T) -> *mut T
    where
        T: Sized,
    {
        let kernel_stack_top = self.get_top();
        let ptr_mut = (kernel_stack_top - core::mem::size_of::<T>()) as *mut T;
        unsafe {
            *ptr_mut = value;
        }
        ptr_mut
    }
    pub fn get_top(&self) -> usize {
        let (_, kernel_stack_top) = kernel_stack_position(self.0);
        kernel_stack_top
    }
}

pub fn trap_cx_bottom_from_lid(lid: usize) -> usize {
    TRAP_CONTEXT_BASE - lid * PAGE_SIZE
}

pub fn ustack_bottom_from_lid(lid: usize) -> usize {
    USER_STACK_BASE + lid * (PAGE_SIZE + USER_STACK_SIZE)
}

// pub struct TaskUserRes {
//     // TODO: recycle lid when the thread exit
//     pub lid: usize, // local thread id for pthread_self()
//     pub process: Weak<ProcessControlBlock>,
// }

// impl TaskUserRes {
//     pub fn new(process: Arc<ProcessControlBlock>, lid: usize, is_alloc_user_res: bool) -> Self {
//         let task_user_res = Self {
//             lid,
//             process: Arc::downgrade(&process),
//         };
//         if is_alloc_user_res {
//             task_user_res.alloc_task_user_res();
//         }
//         task_user_res
//     }

//     pub fn alloc_task_user_res(&self) {
//         let process = self.process.upgrade().unwrap();
//         let mut process_inner = process.acquire_inner_lock();
//         // alloc user stack
//         let ustack_bottom = ustack_bottom_from_tid(self.lid);
//         let ustack_top = ustack_bottom + USER_STACK_SIZE;
//         process_inner.addrspace.insert_framed_area(
//             ".ustack".to_string(),
//             ustack_bottom.into(),
//             ustack_top.into(),
//             MapPermission::R | MapPermission::W | MapPermission::U,
//         );
//         // alloc trap_cx
//         let trap_cx_bottom = trap_cx_bottom_from_tid(self.lid);
//         let trap_cx_top = trap_cx_bottom + PAGE_SIZE;
//         process_inner.addrspace.insert_framed_area(
//             ".trap_cx".to_string(),
//             trap_cx_bottom.into(),
//             trap_cx_top.into(),
//             MapPermission::R | MapPermission::W,
//         );
//     }

//     fn dealloc_user_res(&self) {
//         // dealloc tid
//         let process = self.process.upgrade().unwrap();
//         let mut process_inner = process.acquire_inner_lock();
//         // dealloc ustack manually
//         let ustack_bottom_va: VirtAddr = ustack_bottom_from_tid(self.lid).into();
//         process_inner
//             .addrspace
//             .remove_area_with_start_vpn(ustack_bottom_va.into());
//         // dealloc trap_cx manually
//         let trap_cx_bottom_va: VirtAddr = trap_cx_bottom_from_tid(self.lid).into();
//         process_inner
//             .addrspace
//             .remove_area_with_start_vpn(trap_cx_bottom_va.into());
//     }

//     pub fn trap_cx_user_va(&self) -> usize {
//         trap_cx_bottom_from_tid(self.lid)
//     }

//     pub fn trap_cx_ppn(&self) -> PhysPageNum {
//         let process = self.process.upgrade().unwrap();
//         let process_inner = process.acquire_inner_lock();
//         let trap_cx_bottom_va: VirtAddr = trap_cx_bottom_from_tid(self.lid).into();
//         process_inner
//             .addrspace
//             .translate(trap_cx_bottom_va.into())
//             .unwrap()
//             .ppn()
//     }
//     pub fn ustack_top(&self) -> usize {
//         ustack_bottom_from_tid(self.lid) + USER_STACK_SIZE
//     }
// }

// impl Drop for TaskUserRes {
//     fn drop(&mut self) {
//         self.dealloc_user_res();
//     }
// }
