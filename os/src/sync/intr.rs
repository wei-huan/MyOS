use crate::cpus::Cpu;
use riscv::register::sstatus;

// enable device interrupts
pub fn intr_on() {
    unsafe {
        sstatus::set_sie();
    }
}
// disable device interrupts
pub fn intr_off() {
    unsafe {
        sstatus::clear_sie();
    }
}

// are device interrupts enabled?
pub fn intr_get() -> bool {
    sstatus::read().sie()
}

// Since there may be more than one IntrLock,
// make it an immutable reference to the Cpu struct.
// so, noff is wrapped in UnsafeCell.
pub struct IntrLock<'a> {
    pub cpu: &'a Cpu,
}

impl<'a> Drop for IntrLock<'a> {
    fn drop(&mut self) {
        unsafe {
            self.cpu.unlock();
        }
    }
}
