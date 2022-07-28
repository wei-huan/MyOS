// use super::signal::SignalFlags;
use super::context::TaskContext;
use super::kernel_stack::{kstack_alloc, KernelStack};
use crate::config::{MMAP_BASE, PAGE_SIZE, TRAP_CONTEXT};
use crate::fs::{File, FileClass, FileDescripter, Stdin, Stdout};
use crate::hart_id;
use crate::mm::{
    kernel_token, translated_byte_buffer, translated_refmut, AddrSpace, Permission, PhysPageNum,
    UserBuffer, VirtAddr, KERNEL_SPACE,
};
use crate::syscall::{EBADF, ENOENT, EPERM};
use crate::task::{
    pid_alloc, AuxHeader, PidHandle, SignalActions, SignalFlags, AT_EXECFN, AT_NULL, AT_RANDOM,
};
use crate::trap::{user_trap_handler, TrapContext};
use alloc::string::String;
use alloc::sync::{Arc, Weak};
use alloc::vec;
use alloc::vec::Vec;
use bitflags::bitflags;
use spin::{Mutex, MutexGuard};

bitflags! {
    #[derive(Default)]
    pub struct MMapFlags: usize {
        const MAP_32BIT = 0;
        const MAP_SHARED = 1 << 0;
        const MAP_PRIVATE = 1 << 1;
        const _X2 = 1 << 2;
        const _X3 = 1 << 3;
        const MAP_FIXED = 1 << 4;
        const MAP_ANONYMOUS = 1 << 5;
        const _X6 = 1 << 6;
        const _X7 = 1 << 7;
        const _X8 = 1 << 8;
        const _X9 = 1 << 9;
        const _X10 = 1 << 10;
        const _X11 = 1 << 11;
    }
}

#[derive(Copy, Clone, PartialEq)]
pub enum TaskStatus {
    Ready,
    Running,
    Zombie,
}

pub struct TaskControlBlock {
    // immutable
    pub pid: PidHandle,
    pub kernel_stack: KernelStack,
    // mutable
    inner: Mutex<TaskControlBlockInner>,
}

pub type FDTable = Vec<Option<FileDescripter>>;
pub struct TaskControlBlockInner {
    pub entry_point: usize, // 用户程序入口点 exec会改变
    pub trap_cx_ppn: PhysPageNum,
    pub ustack_bottom: usize,
    pub heap_start: usize,
    pub heap_pointer: usize,
    pub task_cx: TaskContext,
    pub task_status: TaskStatus,
    pub addrspace: AddrSpace,
    pub parent: Option<Weak<TaskControlBlock>>,
    pub children: Vec<Arc<TaskControlBlock>>,
    pub exit_code: i32,
    pub fd_table: FDTable,
    pub current_path: String,
    // mmap area
    pub mmap_area_hint: usize,
    // signals
    pub signals: SignalFlags,
    pub signal_mask: SignalFlags,
    // the signal which is being handling
    pub handling_sig: isize,
    // Signal actions
    pub signal_actions: SignalActions,
    // if the task is killed
    pub killed: bool,
    // if the task is frozen by a signal
    pub frozen: bool,
    pub trap_ctx_backup: Option<TrapContext>,
}

impl TaskControlBlockInner {
    pub fn get_trap_cx(&self) -> &'static mut TrapContext {
        self.trap_cx_ppn.get_mut()
    }
    pub fn get_user_token(&self) -> usize {
        self.addrspace.token()
    }
    fn get_status(&self) -> TaskStatus {
        self.task_status
    }
    pub fn is_zombie(&self) -> bool {
        self.get_status() == TaskStatus::Zombie
    }
    pub fn alloc_fd(&mut self) -> usize {
        if let Some(fd) = (0..self.fd_table.len()).find(|fd| self.fd_table[*fd].is_none()) {
            fd
        } else {
            self.fd_table.push(None);
            self.fd_table.len() - 1
        }
    }
    pub fn get_work_path(&self) -> String {
        self.current_path.clone()
    }
    // pub fn have_children(&self) -> bool {
    //     self.children.len() > 0
    // }
}

impl TaskControlBlock {
    pub fn acquire_inner_lock(&self) -> MutexGuard<TaskControlBlockInner> {
        self.inner.lock()
    }
    // only for initproc
    pub fn new(elf_data: &[u8]) -> Self {
        // memory_set with elf program headers/trampoline/trap context/user stack
        let (addrspace, heap_start, ustack_base, entry_point, _) =
            AddrSpace::create_user_space(elf_data);
        let trap_cx_ppn = addrspace
            .translate(VirtAddr::from(TRAP_CONTEXT).into())
            .unwrap()
            .ppn();
        // allocate a pid
        let pid_handle = pid_alloc();
        let kernel_stack = kstack_alloc();
        let kernel_stack_top = kernel_stack.get_top();
        let task = Self {
            pid: pid_handle,
            kernel_stack,
            inner: Mutex::new(TaskControlBlockInner {
                entry_point,
                trap_cx_ppn,
                ustack_bottom: ustack_base,
                heap_start,
                heap_pointer: heap_start,
                task_cx: TaskContext::goto_trap_return(kernel_stack_top),
                task_status: TaskStatus::Ready,
                addrspace,
                parent: None,
                children: Vec::new(),
                exit_code: 0,
                fd_table: vec![
                    // 0 -> stdin
                    Some(FileDescripter::new(
                        false,
                        FileClass::Abstr(Arc::new(Stdin)),
                    )),
                    // 1 -> stdout
                    Some(FileDescripter::new(
                        false,
                        FileClass::Abstr(Arc::new(Stdout)),
                    )),
                    // 2 -> stderr
                    Some(FileDescripter::new(
                        false,
                        FileClass::Abstr(Arc::new(Stdout)),
                    )),
                ],
                current_path: String::from("/"),
                mmap_area_hint: MMAP_BASE,
                signals: SignalFlags::empty(),
                signal_mask: SignalFlags::empty(),
                handling_sig: -1,
                signal_actions: SignalActions::default(),
                killed: false,
                frozen: false,
                trap_ctx_backup: None,
            }),
        };
        // prepare TrapContext in user space
        let trap_cx = task.acquire_inner_lock().get_trap_cx();
        *trap_cx = TrapContext::app_init_context(
            entry_point,
            ustack_base,
            kernel_token(),
            kernel_stack_top,
            user_trap_handler as usize,
            hart_id(),
        );
        task
    }
    pub fn exec(self: &Arc<Self>, elf_data: &[u8], args: Vec<String>) {
        // memory_set with elf program headers/trampoline/trap context/user stack
        let (addrspace, heap_start, mut user_sp, entry_point, mut auxv) =
            AddrSpace::create_user_space(elf_data);
        let token = addrspace.token();
        let trap_cx_ppn = addrspace
            .translate(VirtAddr::from(TRAP_CONTEXT).into())
            .unwrap()
            .ppn();

        ////////////// envp[] ///////////////////
        let mut env: Vec<String> = Vec::new();
        env.push(String::from("SHELL=/user_shell"));
        env.push(String::from("PWD=/"));
        env.push(String::from("USER=root"));
        env.push(String::from("MOTD_SHOWN=pam"));
        env.push(String::from("LANG=C.UTF-8"));
        env.push(String::from(
            "INVOCATION_ID=e9500a871cf044d9886a157f53826684",
        ));
        env.push(String::from("TERM=vt220"));
        env.push(String::from("SHLVL=2"));
        env.push(String::from("JOURNAL_STREAM=8:9265"));
        env.push(String::from("OLDPWD=/root"));
        env.push(String::from("_=busybox"));
        env.push(String::from("LOGNAME=root"));
        env.push(String::from("HOME=/"));
        env.push(String::from("PATH=/"));
        env.push(String::from("LD_LIBRARY_PATH=/"));
        let mut envp: Vec<usize> = (0..=env.len()).collect();
        envp[env.len()] = 0;

        for i in 0..env.len() {
            user_sp -= env[i].len() + 1;
            envp[i] = user_sp;
            let mut p = user_sp;
            // write chars to [user_sp, user_sp + len]
            for c in env[i].as_bytes() {
                *translated_refmut(token, p as *mut u8) = *c;
                p += 1;
            }
            *translated_refmut(token, p as *mut u8) = 0;
        }
        // make the user_sp aligned to 8B for k210 platform
        user_sp -= user_sp % core::mem::size_of::<usize>();

        ////////////// argv[] ///////////////////
        let mut argv: Vec<usize> = (0..=args.len()).collect();
        argv[args.len()] = 0;
        for i in 0..args.len() {
            user_sp -= args[i].len() + 1;
            // println!("user_sp {:X}", user_sp);
            argv[i] = user_sp;
            let mut p = user_sp;
            // write chars to [user_sp, user_sp + len]
            for c in args[i].as_bytes() {
                *translated_refmut(token, p as *mut u8) = *c;
                // print!("({})",*c as char);
                p += 1;
            }
            *translated_refmut(token, p as *mut u8) = 0;
        }
        // make the user_sp aligned to 8B for k210 platform
        user_sp -= user_sp % core::mem::size_of::<usize>();

        ////////////// platform String ///////////////////
        let platform = "RISC-V64";
        user_sp -= platform.len() + 1;
        user_sp -= user_sp % core::mem::size_of::<usize>();
        let mut p = user_sp;
        for c in platform.as_bytes() {
            *translated_refmut(token, p as *mut u8) = *c;
            p += 1;
        }
        *translated_refmut(token, p as *mut u8) = 0;

        ////////////// rand bytes ///////////////////
        user_sp -= 16;
        p = user_sp;
        auxv.push(AuxHeader {
            aux_type: AT_RANDOM,
            value: user_sp,
        });
        for i in 0..0xf {
            *translated_refmut(token, p as *mut u8) = i as u8;
            p += 1;
        }

        ////////////// padding //////////////////////
        user_sp -= user_sp % 16;

        ////////////// auxv[] //////////////////////
        auxv.push(AuxHeader {
            aux_type: AT_EXECFN,
            value: argv[0],
        }); // file name
        auxv.push(AuxHeader {
            aux_type: AT_NULL,
            value: 0,
        }); // end
        user_sp -= auxv.len() * core::mem::size_of::<AuxHeader>();
        let auxv_base = user_sp;
        // println!("[auxv]: base 0x{:X}", auxv_base);
        for i in 0..auxv.len() {
            // println!("[auxv]: {:?}", auxv[i]);
            let addr = user_sp + core::mem::size_of::<AuxHeader>() * i;
            *translated_refmut(token, addr as *mut usize) = auxv[i].aux_type;
            *translated_refmut(token, (addr + core::mem::size_of::<usize>()) as *mut usize) =
                auxv[i].value;
        }

        ////////////// *envp [] //////////////////////
        user_sp -= (env.len() + 1) * core::mem::size_of::<usize>();
        let envp_base = user_sp;
        *translated_refmut(
            token,
            (user_sp + core::mem::size_of::<usize>() * (env.len())) as *mut usize,
        ) = 0;
        for i in 0..env.len() {
            *translated_refmut(
                token,
                (user_sp + core::mem::size_of::<usize>() * i) as *mut usize,
            ) = envp[i];
        }

        ////////////// *argv [] //////////////////////
        user_sp -= (args.len() + 1) * core::mem::size_of::<usize>();
        let argv_base = user_sp;
        *translated_refmut(
            token,
            (user_sp + core::mem::size_of::<usize>() * (args.len())) as *mut usize,
        ) = 0;
        for i in 0..args.len() {
            *translated_refmut(
                token,
                (user_sp + core::mem::size_of::<usize>() * i) as *mut usize,
            ) = argv[i];
        }

        ////////////// argc //////////////////////
        user_sp -= core::mem::size_of::<usize>();
        *translated_refmut(token, user_sp as *mut usize) = args.len();

        // **** access current TCB exclusively
        let mut inner = self.acquire_inner_lock();
        // set new entry_point
        inner.entry_point = entry_point;
        // substitute addrspace
        inner.addrspace = addrspace;
        // update trap_cx ppn
        inner.trap_cx_ppn = trap_cx_ppn;
        // update heap_start
        inner.heap_start = heap_start;
        // update heap_pointer
        inner.heap_pointer = heap_start;

        // initialize trap_cx
        let mut trap_cx = TrapContext::app_init_context(
            entry_point,
            user_sp,
            KERNEL_SPACE.lock().token(),
            self.kernel_stack.get_top(),
            user_trap_handler as usize,
            hart_id(),
        );
        trap_cx.x[10] = args.len();
        trap_cx.x[11] = argv_base;
        trap_cx.x[12] = envp_base;
        trap_cx.x[13] = auxv_base;
        *inner.get_trap_cx() = trap_cx;
    }
    pub fn fork(self: &Arc<TaskControlBlock>) -> Arc<TaskControlBlock> {
        // ---- hold parent PCB lock
        let mut parent_inner = self.acquire_inner_lock();
        // copy user space(include trap context)
        let addrspace = AddrSpace::from_existed_user(&parent_inner.addrspace);
        let trap_cx_ppn = addrspace
            .translate(VirtAddr::from(TRAP_CONTEXT).into())
            .unwrap()
            .ppn();
        // alloc a pid and a kernel stack in kernel space
        let pid_handle = pid_alloc();
        let kernel_stack = kstack_alloc();
        let kernel_stack_top = kernel_stack.get_top();
        // copy fd table
        let mut new_fd_table: FDTable = Vec::new();
        for fd in parent_inner.fd_table.iter() {
            if let Some(file) = fd {
                new_fd_table.push(Some(file.clone()));
            } else {
                new_fd_table.push(None);
            }
        }
        let task_control_block = Arc::new(TaskControlBlock {
            pid: pid_handle,
            kernel_stack,
            inner: Mutex::new(TaskControlBlockInner {
                entry_point: parent_inner.entry_point,
                trap_cx_ppn,
                ustack_bottom: parent_inner.ustack_bottom,
                heap_start: parent_inner.heap_start,
                heap_pointer: parent_inner.heap_pointer,
                task_cx: TaskContext::goto_trap_return(kernel_stack_top),
                task_status: TaskStatus::Ready,
                addrspace,
                parent: Some(Arc::downgrade(self)),
                children: Vec::new(),
                exit_code: 0,
                fd_table: new_fd_table,
                current_path: parent_inner.current_path.clone(),
                mmap_area_hint: parent_inner.mmap_area_hint,
                signals: SignalFlags::empty(),
                // inherit the signal_mask and signal_action
                signal_mask: parent_inner.signal_mask,
                handling_sig: -1,
                signal_actions: parent_inner.signal_actions.clone(),
                killed: false,
                frozen: false,
                trap_ctx_backup: None,
            }),
        });
        // add child
        parent_inner.children.push(task_control_block.clone());
        // modify kernel_sp in trap_cx
        // **** access child PCB exclusively
        let trap_cx = task_control_block.acquire_inner_lock().get_trap_cx();
        trap_cx.kernel_sp = kernel_stack_top;
        // return
        task_control_block
        // **** release child PCB
        // ---- release parent PCB
    }
    pub fn getpid(&self) -> usize {
        self.pid.0
    }
    pub fn get_parent(&self) -> Option<Arc<TaskControlBlock>> {
        let inner = self.acquire_inner_lock();
        inner.parent.as_ref().unwrap().upgrade()
    }
    // initproc won't call sys_getppid
    pub fn getppid(&self) -> usize {
        self.get_parent().unwrap().pid.0
    }
    pub fn mmap(
        &self,
        mut start: usize,
        mut length: usize,
        prot: usize,
        flags: usize,
        fd: isize,
        offset: usize,
    ) -> isize {
        log::debug!(
            "start {:#X}, length: {:#X}, fd: {:#X}, offset: {:#X}",
            start,
            length,
            fd,
            offset
        );
        let mut inner = self.acquire_inner_lock();
        let token = inner.get_user_token();
        // prot<<1 is equal to  meaning of Permission
        let mmap_perm = Permission::from_bits((prot << 1) as u8).unwrap() | Permission::U;
        // need hint
        if start == 0 {
            start = inner.mmap_area_hint;
            log::debug!("mmap need hint start before map: {:#X}", start);
            inner.mmap_area_hint = inner
                .addrspace
                .create_mmap_section(start, length, mmap_perm)
                .into();
            log::debug!("mmap need hint hint after map: {:#X}", inner.mmap_area_hint);
        }
        // another mmaping already exist there, need to picks a new address depending on the hint
        else if inner
            .addrspace
            .is_mmap_section_exist(VirtAddr::from(start).floor())
        {
            start = inner.mmap_area_hint;
            log::debug!("mmap need to pick new place start before map: {:#X}", start);
            inner.mmap_area_hint = inner
                .addrspace
                .create_mmap_section(start, length, mmap_perm)
                .into();
            log::debug!(
                "mmap need to pick new place start after map: {:#X}",
                inner.mmap_area_hint
            );
        }
        // no conflict, just map it
        else {
            log::trace!("mmap start before map: {:#X}", start);
            inner.mmap_area_hint = inner
                .addrspace
                .create_mmap_section(start, length, mmap_perm)
                .into();
            log::trace!("mmap hint after map: {:#X}", inner.mmap_area_hint);
        }

        // File Content Copy
        let fd_table = inner.fd_table.clone();
        let mmap_flag = MMapFlags::from_bits(flags).unwrap();
        if fd < 0 || mmap_flag.contains(MMapFlags::MAP_ANONYMOUS) {
            log::trace!("mmap here no need file");
            return start as isize;
        }
        if fd as usize >= fd_table.len() {
            return -EBADF;
        }
        if let Some(file) = &fd_table[fd as usize] {
            match &file.fclass {
                FileClass::File(f) => {
                    if !f.readable() {
                        return -EPERM;
                    }
                    f.set_offset(offset);
                    log::debug! {"The va_start is {:#?}, offset of file is {:#X?}, file_size: {:#X?}", VirtAddr::from(start), offset, f.get_size()};
                    let read_len = f.read(UserBuffer::new(translated_byte_buffer(
                        token, start as _, length,
                    )));
                    log::debug! {"read {:#X?} bytes", read_len};
                    return start as isize;
                }
                _ => {
                    return -ENOENT;
                }
            };
        } else {
            return -ENOENT;
        };
    }

    pub fn munmap(&self, start: usize, _length: usize) -> isize {
        let mut inner = self.acquire_inner_lock();
        inner
            .addrspace
            .remove_mmap_area_with_start_vpn(VirtAddr::from(start).into());
        0
    }
}
