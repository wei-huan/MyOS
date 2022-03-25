use crate::trap::trap_return;
use riscv::register::sstatus::{self, SPP};

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct TaskContext {
    ra: usize,
    sp: usize,
    sstatus: usize,
    s: [usize; 12],
}

impl TaskContext {
    // pub fn zero_init() -> Self {
    //     let sstatus = sstatus::read();
    //     Self {
    //         ra: 0,
    //         sp: 0,
    //         sstatus,
    //         s: [0; 12],
    //     }
    // }
    #[allow(unused)]
    pub fn get_sp(&self) -> usize{
        self.sp
    }
    pub fn goto_trap_return(kstack_ptr: usize) -> Self {
        let mut sstatus = sstatus::read();
        sstatus.set_spp(SPP::User);
        let sstatus = sstatus.bits();
        Self {
            ra: trap_return as usize,
            sp: kstack_ptr,
            sstatus,
            s: [0; 12],
        }
    }
}
