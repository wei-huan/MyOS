use crate::config::MMAP_BASE;
use crate::fs::{File, FileClass, FileDescripter, Stdin, Stdout};
use crate::hart_id;
use crate::mm::{
    kernel_token, translated_byte_buffer, translated_refmut, AddrSpace, MMapFlags, MapPermission,
    PhysPageNum, UserBuffer, VirtAddr, KERNEL_SPACE,
};
use crate::syscall::{EBADF, ENOENT, EPERM};
use crate::task::{
    pid_alloc, AuxHeader, PidHandle, SignalActions, SignalFlags, TaskControlBlock, AT_EXECFN,
    AT_NULL, AT_RANDOM,
};
use crate::trap::{user_trap_handler, TrapContext};
use alloc::string::String;
use alloc::sync::{Arc, Weak};
use alloc::vec;
use alloc::vec::Vec;
use spin::{Mutex, MutexGuard};

pub struct ProcessControlBlock {
    // immutable
    pub pid: PidHandle,
    // mutable
    inner: Mutex<ProcessControlBlockInner>,
}

pub type FDTable = Vec<Option<FileDescripter>>;
pub struct ProcessControlBlockInner {
    pub entry_point: usize,
    pub heap_start: usize,
    pub heap_pointer: usize,
    pub addrspace: AddrSpace,
    pub parent: Option<Weak<ProcessControlBlock>>,
    pub children: Vec<Arc<ProcessControlBlock>>,
    pub tasks: Vec<Arc<TaskControlBlock>>,
    pub exit_code: i32,
    pub fd_table: FDTable,
    pub current_path: String,
    pub mmap_area_hint: usize,
    pub signals: SignalFlags,
    pub signal_actions: SignalActions,
}

impl ProcessControlBlockInner {
    pub fn get_user_token(&self) -> usize {
        self.addrspace.token()
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
    pub fn thread_count(&self) -> usize {
        self.tasks.len()
    }
}

impl ProcessControlBlock {
    pub fn acquire_inner_lock(&self) -> MutexGuard<ProcessControlBlockInner> {
        self.inner.lock()
    }
    // only for initproc
    pub fn new(elf_data: &[u8]) -> Self {
        // memory_set with elf program headers/trampoline/trap context/user stack
        let (addrspace, heap_start, _, entry_point, _) = AddrSpace::create_user_space(elf_data);
        // allocate a pid
        let pid_handle = pid_alloc();
        let process = Self {
            pid: pid_handle,
            inner: Mutex::new(ProcessControlBlockInner {
                entry_point,
                heap_start,
                heap_pointer: heap_start,
                addrspace,
                parent: None,
                children: Vec::new(),
                tasks: Vec::new(),
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
                signal_actions: SignalActions::default(),
            }),
        };
        let main_task = TaskControlBlock::new(Arc::new(process), 0, true);
        let trap_cx_ppn = main_task.acquire_inner_lock().trap_cx_ppn;
        process
    }
    pub fn exec(self: &Arc<Self>, elf_data: &[u8], args: Vec<String>) {
        // assert_eq!(self.acquire_inner_lock().thread_count(), 1, "process can't execve with multithread");
        // memory_set with elf program headers/trampoline/trap context/user stack
        let (addrspace, heap_start, mut user_sp, entry_point, mut auxv) =
            AddrSpace::create_user_space(elf_data);
        let token = addrspace.token();
        let trap_cx_ppn = addrspace
            .translate(VirtAddr::from(TRAP_CONTEXT).into())
            .unwrap()
            .ppn();

        ////////////// *envp[] ///////////////////
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

        ////////////// *argv[] ///////////////////
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

        ////////////// *platform String ///////////////////
        let platform = "RISC-V64";
        user_sp -= platform.len() + 1;
        user_sp -= user_sp % core::mem::size_of::<usize>();
        let mut p = user_sp;
        for c in platform.as_bytes() {
            *translated_refmut(token, p as *mut u8) = *c;
            p += 1;
        }
        *translated_refmut(token, p as *mut u8) = 0;

        ////////////// *rand bytes ///////////////////
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

        ////////////// *padding //////////////////////
        user_sp -= user_sp % 16;

        ////////////// *auxv[] //////////////////////
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

        ////////////// *argc //////////////////////
        user_sp -= core::mem::size_of::<usize>();
        *translated_refmut(token, user_sp as *mut usize) = args.len();

        // **** access current TCB exclusively
        let mut inner = self.acquire_inner_lock();
        // set new entry_point
        inner.entry_point = entry_point;
        // substitute addrspace
        inner.addrspace = addrspace;
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
    pub fn fork(self: &Arc<ProcessControlBlock>) -> Arc<ProcessControlBlock> {
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
        let task_control_block = Arc::new(ProcessControlBlock {
            pid: pid_handle,
            inner: Mutex::new(ProcessControlBlockInner {
                entry_point: parent_inner.entry_point,
                heap_start: parent_inner.heap_start,
                heap_pointer: parent_inner.heap_pointer,
                addrspace,
                parent: Some(Arc::downgrade(self)),
                children: Vec::new(),
                exit_code: 0,
                fd_table: new_fd_table,
                current_path: parent_inner.current_path.clone(),
                mmap_area_hint: parent_inner.mmap_area_hint,
                // inherit the signals and signal_action
                signals: SignalFlags::empty(),
                signal_actions: parent_inner.signal_actions.clone(),
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
    pub fn get_parent(&self) -> Option<Arc<ProcessControlBlock>> {
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
        length: usize,
        prot: usize,
        flags: usize,
        fd: isize,
        offset: usize,
    ) -> isize {
        let mut inner = self.acquire_inner_lock();
        let token = inner.get_user_token();
        // prot << 1 is equal to meaning of MapPermission
        let mmap_perm = MapPermission::from_bits((prot << 1) as u8).unwrap() | MapPermission::U;
        let mmap_flag = MMapFlags::from_bits(flags).unwrap();
        log::trace!(
            "start {:#X}, length: {:#X}, fd: {:#X}, offset: {:#X}, flags: {:?}, mmap_flag: {:?}",
            start,
            length,
            fd,
            offset,
            mmap_perm,
            mmap_flag
        );
        /* mmap section */
        // need hint
        if start == 0 {
            start = inner.mmap_area_hint;
            log::trace!("mmap need hint start before map: {:#X}", start);
            inner.mmap_area_hint = inner
                .addrspace
                .create_mmap_section(start, length, mmap_perm)
                .into();
            log::trace!("mmap need hint hint after map: {:#X}", inner.mmap_area_hint);
        }
        // another mmaping already exist there, but need to place the mapping at exactly that address.
        else if inner.addrspace.is_mmap_section_conflict(start, length)
            && mmap_flag.contains(MMapFlags::MAP_FIXED)
        {
            // adjust mmap section
            log::trace!("start fixing mmap conflict");
            inner.addrspace.fix_mmap_section_conflict(start, length);

            // map mmap section at fixed place
            log::trace!("mmap at fixed place start before map: {:#X}", start);
            inner.mmap_area_hint = inner
                .addrspace
                .create_mmap_section(start, length, mmap_perm)
                .into();
            log::debug!(
                "mmap at fixed place hint after map: {:#X}",
                inner.mmap_area_hint
            );
        }
        // no conflict, just map it
        else if mmap_flag.contains(MMapFlags::MAP_FIXED) {
            log::trace!("mmap start before map: {:#X}", start);
            // TODO mmap_area_hint new appropriate?
            inner.mmap_area_hint = inner
                .addrspace
                .create_mmap_section(start, length, mmap_perm)
                .into();
            log::debug!("mmap hint after map: {:#X}", inner.mmap_area_hint);
        }
        // have conflict, but can pick a new place to map
        else {
            start = inner.mmap_area_hint;
            log::trace!("mmap need to pick new place start before map: {:#X}", start);
            inner.mmap_area_hint = inner
                .addrspace
                .create_mmap_section(start, length, mmap_perm)
                .into();
            log::trace!(
                "mmap need to pick new place hint after map: {:#X}",
                inner.mmap_area_hint
            );
        }

        /* File Content Copy if need*/
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
                    log::trace! {"The va_start is {:#?}, offset of file is {:#X?}, file_size: {:#X?}", VirtAddr::from(start), offset, f.get_size()};
                    let read_len = f.read(UserBuffer::new(translated_byte_buffer(
                        token, start as _, length,
                    )));
                    log::trace! {"read {:#X?} bytes", read_len};
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
