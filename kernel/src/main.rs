#![no_std]
#![no_main]
#![feature(panic_info_message)]
#![feature(alloc_error_handler)]

extern crate alloc;
extern crate fdt;

#[macro_use]
mod console;
mod config;
mod cpu;
mod drivers;
mod dt;
mod fs;
mod lang_items;
mod logging;
mod mm;
#[cfg(feature = "opensbi")]
mod opensbi;
mod platform;
#[cfg(feature = "rustsbi")]
mod rustsbi;
mod scheduler;
mod sync;
mod syscall;
mod task;
mod timer;
mod trap;
mod utils;

use crate::cpu::SMP_START;
use core::arch::global_asm;
use core::sync::atomic::Ordering;

global_asm!(include_str!("entry.asm"));

fn clear_bss() {
    extern "C" {
        fn sbss();
        fn ebss();
    }
    (sbss as usize..ebss as usize).for_each(|a| unsafe { (a as *mut u8).write_volatile(0) })
}

// qemu opensbi
// #[cfg(all(feature = "qemu", feature = "opensbi"))]
#[no_mangle]
fn os_main(hartid: usize, dtb_ptr: *mut u8) {
    if !SMP_START.load(Ordering::Acquire) {
        clear_bss();
        println!("fdt: 0x{:X}", dtb_ptr as usize);
        dt::init(dtb_ptr);
        logging::init();
        mm::boot_init();
        logging::show_machine_sbi_os_info();

        scheduler::add_apps();
        trap::init();
        timer::init();
        // SMP_START will turn to true in this function
        cpu::boot_all_harts(hartid);
        log::info!("here 0");
        scheduler::schedule();
    } else {
        trap::init();
        log::info!("here 0");
        mm::init();
        timer::init();
        log::info!("here 1");
        scheduler::schedule();
    }
}

// qemu rustsbi
#[cfg(all(feature = "qemu", feature = "rustsbi"))]
#[no_mangle]
fn os_main(hartid: usize, dtb_ptr: *const u8) {
    if !SMP_START.load(Ordering::Acquire) {
        clear_bss();
        println!("dtb_ptr {}", dtb_ptr as usize);
        dt::init(dtb_ptr);
        logging::init();
        mm::boot_init();
        logging::show_machine_sbi_os_info();
        // scheduler::add_apps();
        trap::init();
        timer::init();
        // SMP_START will turn to true in this function
        cpu::boot_all_harts();
        log::info!("here 4");
        loop {}
    } else {
        while !SMP_START.load(Ordering::Acquire) {}
        trap::init();
        mm::init();
        log::info!("hart{} boot sucessfully", hartid);
        timer::init();
        loop {}
    }
}

// k210 rustsbi
#[cfg(all(feature = "k210", feature = "rustsbi"))]
#[no_mangle]
fn os_main(hartid: usize, dtb_ptr: *mut u8) {
    if hartid == 0 {
        clear_bss();
        println!("here 0");
        dt::init(dtb_ptr);
        logging::init();
        mm::boot_init();
        logging::show_machine_sbi_os_info();
        // scheduler::add_apps();
        trap::init();
        // timer::init();
        // SMP_START will turn to true in this function
        cpu::boot_all_harts(hartid);
        println!("here 4");
        loop {}
    } else {
        trap::init();
        mm::init();
        timer::init();
        loop {}
    }
}

// k210 opensbi
#[cfg(all(feature = "k210", feature = "opensbi"))]
#[no_mangle]
fn os_main(hartid: usize, dtb_ptr: *mut u8) {
    if hartid == 0 {
        clear_bss();
        println!("here 0");
        dt::init(dtb_ptr);
        logging::init();
        mm::boot_init();
        logging::show_machine_sbi_os_info();
        // scheduler::add_apps();
        trap::init();
        // timer::init();
        // SMP_START will turn to true in this function
        cpu::boot_all_harts(hartid);
        println!("here 4");
        loop {}
    } else {
        trap::init();
        mm::init();
        timer::init();
        loop {}
    }
}
