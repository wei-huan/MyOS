use riscv::register::{mtvec::TrapMode, scause, stval, stvec};

pub fn entry_init() {
    set_kernel_entry();
}

pub fn set_kernel_entry() {
    unsafe {
        stvec::write(trap_from_kernel as usize, TrapMode::Direct);
    }
}

#[no_mangle]
pub fn trap_from_kernel() -> ! {
    use riscv::register::sepc;
    println!("stval = {:#?}, sepc = 0x{:X}", stval::read(), sepc::read());
    panic!("a trap {:?} from kernel!", scause::read().cause());
}
