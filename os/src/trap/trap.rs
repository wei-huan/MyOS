use crate::config::{TRAMPOLINE, TRAP_CONTEXT};
use crate::sync::interrupt_on;
use crate::timer::set_next_trigger;
use core::arch::global_asm;
use log::*;
use riscv::register::{
    mtvec::TrapMode,
    scause::{self, Exception, Interrupt, Trap},
    sepc, sscratch, stval, stvec,
};

global_asm!(include_str!("trap.S"));

pub fn set_kernel_trap_entry() {
    extern "C" {
        fn __super_alltraps();
    }
    unsafe {
        stvec::write(__super_alltraps as usize, TrapMode::Direct);
    }
}

#[no_mangle]
pub fn kernel_trap_handler() {
    let scause = scause::read();
    match scause.cause() {
        Trap::Interrupt(Interrupt::SupervisorTimer) => {
            // interrupt_on();
            // set_next_trigger();
            info!("trap sscratch: {}", sscratch::read());
            strap_return()
        }
        _ => {
            println!("stval = {:#?}, sepc = 0x{:X}", stval::read(), sepc::read());
            panic!("a trap {:?} from kernel!", scause.cause());
        }
    }
}

#[no_mangle]
fn strap_return() -> ! {
    extern "C" {
        fn __super_restore();
    }
    unsafe { __super_restore() }
    unreachable!();
}

fn set_user_trap_entry() {
    unsafe {
        stvec::write(TRAMPOLINE as usize, TrapMode::Direct);
    }
}

#[no_mangle]
pub fn app_trap_handler() -> ! {
    user_trap_return();
}

#[no_mangle]
pub fn user_trap_return() -> ! {
    unreachable!();
}
