use crate::cpu::{current_task, current_user_token};
use crate::fs::{open, DiskInodeType, File, FileClass, FileDescripter, OpenFlags};
use crate::mm::{translated_byte_buffer, translated_str, UserBuffer};
use alloc::sync::Arc;

const AT_FDCWD: isize = -100;
pub const FD_LIMIT: usize = 128;

pub fn sys_write(fd: usize, buf: *const u8, len: usize) -> isize {
    let token = current_user_token();
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if let Some(file) = &inner.fd_table[fd] {
        let file: Arc<dyn File + Send + Sync> = match &file.fclass {
            FileClass::Abstr(f) => f.clone(),
            FileClass::File(f) => {
                /*print!("\n");*/
                f.clone()
            }
            _ => return -1,
        };
        if !file.writable() {
            return -1;
        }
        drop(inner);
        let size = file.write(UserBuffer::new(translated_byte_buffer(token, buf, len)));
        if fd == 2 {
            // str::replace(translated_str(token, buf).as_str(), "\n", "\\n");
        }
        size as isize
    } else {
        -1
    }
}

pub fn sys_read(fd: usize, buf: *const u8, len: usize) -> isize {
    let token = current_user_token();
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if let Some(file) = &inner.fd_table[fd] {
        let file: Arc<dyn File + Send + Sync> = match &file.fclass {
            FileClass::Abstr(f) => f.clone(),
            FileClass::File(f) => {
                /*print!("\n");*/
                f.clone()
            }
            _ => return -1,
        };
        if !file.readable() {
            return -1;
        }
        // release current task PCB inner manually to avoid multi-borrow
        drop(inner);
        // release current task PCB manually to avoid Arc::strong_count grow
        drop(task);
        let size = file.read(UserBuffer::new(translated_byte_buffer(token, buf, len)));
        size as isize
    } else {
        -1
    }
}

// pub fn sys_open(path: *const u8, flags: u32) -> isize {
//     let task = current_task().unwrap();
//     let token = current_user_token();
//     let path = translated_str(token, path);
//     if let Some(inode) = open_file(path.as_str(), OpenFlags::from_bits(flags).unwrap()) {
//         let mut inner = task.inner_exclusive_access();
//         let fd = inner.alloc_fd();
//         inner.fd_table[fd] = Some(inode);
//         fd as isize
//     } else {
//         -1
//     }
// }

// TODO:文件所有权
pub fn sys_open_at(dirfd: isize, path: *const u8, flags: u32, _mode: u32) -> isize {
    let task = current_task().unwrap();
    let token = current_user_token();
    // 这里传入的地址为用户的虚地址，因此要使用用户的虚地址进行映射
    let path = translated_str(token, path);
    let mut inner = task.inner_exclusive_access();

    let oflags = OpenFlags::from_bits(flags).unwrap();
    if dirfd == AT_FDCWD {
        if let Some(inode) = open(
            inner.get_work_path().as_str(),
            path.as_str(),
            oflags,
            DiskInodeType::File,
        ) {
            let fd = inner.alloc_fd();
            inner.fd_table[fd] = Some(FileDescripter::new(
                oflags.contains(OpenFlags::CLOEXEC),
                FileClass::File(inode),
            ));
            fd as isize
        } else {
            //panic!("open failed");
            -1
        }
    } else {
        let fd_usz = dirfd as usize;
        if fd_usz >= inner.fd_table.len() && fd_usz > FD_LIMIT {
            return -1;
        }
        if let Some(file) = &inner.fd_table[fd_usz] {
            match &file.fclass {
                FileClass::File(f) => {
                    //let oflags = OpenFlags::from_bits(flags).unwrap();
                    // 需要新建文件
                    if oflags.contains(OpenFlags::CREATE) {
                        if let Some(tar_f) = f.create(path.as_str(), DiskInodeType::File) {
                            let fd = inner.alloc_fd();
                            inner.fd_table[fd] = Some(FileDescripter::new(
                                oflags.contains(OpenFlags::CLOEXEC),
                                FileClass::File(tar_f),
                            ));
                            return fd as isize;
                        } else {
                            //panic!("open failed");
                            return -1;
                        }
                    }
                    // 正常打开文件
                    if let Some(tar_f) = f.find(path.as_str(), oflags) {
                        let fd = inner.alloc_fd();
                        inner.fd_table[fd] = Some(FileDescripter::new(
                            oflags.contains(OpenFlags::CLOEXEC),
                            FileClass::File(tar_f),
                        ));
                        fd as isize
                    } else {
                        //panic!("open failed");
                        return -1;
                    }
                }
                _ => return -1, // 如果是抽象类文件，不能open
            }
        } else {
            return -1;
        }
    }
}

pub fn sys_close(fd: usize) -> isize {
    let task = current_task().unwrap();
    let mut inner = task.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if inner.fd_table[fd].is_none() {
        return -1;
    }
    inner.fd_table[fd].take();
    0
}
