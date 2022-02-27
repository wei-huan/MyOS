#![no_std]
#![no_main]
#![feature(panic_info_message)]
#![feature(alloc_error_handler)]

extern crate alloc;

#[macro_use]
mod console;
mod boards;
mod config;
mod kernelvec;
mod memlayout;
mod start;
mod cpus;
mod lang_items;
mod mm;
mod sbi;
mod sync;

use core::arch::asm;
use core::arch::global_asm;
use core::sync::atomic::{AtomicBool, Ordering};
// use boards::CPU_NUM;
global_asm!(include_str!("entry.asm"));

#[no_mangle]
fn clear_bss() {
    extern "C" {
        fn sbss();
        fn ebss();
    }
    (sbss as usize..ebss as usize).for_each(|a| unsafe { (a as *mut u8).write_volatile(0) })
}

static STARTED: AtomicBool = AtomicBool::new(false);

#[no_mangle]
fn os_main(hartid: usize) {
    unsafe {
        if hartid == 0 {
            clear_bss();
            // for i in 1..2 {
            //     let mask: usize = 1 << i;
            //     sbi::send_ipi(&mask as *const _ as usize);
            // }
            asm!("fence");
            STARTED.store(true, Ordering::SeqCst);
            println!("cpu{}", hartid);
        } else {
            while !STARTED.load(Ordering::SeqCst) {}
            asm!("fence");
            println!("cpu{}", hartid);
            panic!("from hart{}", hartid);
        }
        loop {}
    }
}

// #[no_mangle]
// fn os_main(hartid: usize) {
//         if STARTED.load(Ordering::SeqCst) == false {
//             println!("cpu{}", hartid);
//             clear_bss();
//             // wake up;
//             for i in 0..2 {
//                 if hartid != i {
//                     println!("wake hart{}", i);
//                     let mask: usize = 1 << i;
//                     sbi::send_ipi(&mask as *const _ as usize);
//                 }
//             }
//             // asm!("fence");
//             STARTED.store(true, Ordering::SeqCst);
//         } else {
//             println!("here waiting");
//             while !STARTED.load(Ordering::SeqCst) {}
//             // asm!("fence");
//             println!("hart{} now wake", hartid);
//         }
//         loop {}
//         // panic!("from hart{}", hartid);
// }

// #[no_mangle]
// fn os_main(hartid: usize) {
//     println!("cpu{}", hartid);
//     if STARTED.load(Ordering::SeqCst) == false {
//         clear_bss();
//         for i in 0..2 {
//             if hartid != i {
//                 // println!("wake hart{}", i);
//                 let mask: usize = 1 << i;
//                 sbi::send_ipi(&mask as *const _ as usize);
//             }
//         }
//         unsafe{asm!("fence");}
//         STARTED.store(true, Ordering::SeqCst);
//     } else {
//         // println!("here waiting");
//         while !STARTED.load(Ordering::SeqCst) {}
//         unsafe{asm!("fence");}
//         println!("hart{} now wake", hartid);
//     }
//     panic!("from hart{}", hartid);
//     loop {}
// }
