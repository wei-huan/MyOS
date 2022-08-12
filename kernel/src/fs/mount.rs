//use core::f32::MANTISSA_DIGITS;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use lazy_static::*;
use spin::Mutex;

const MNT_MAXLEN: usize = 16;

pub struct MountTable {
    mnt_list: Vec<(String, String, String)>, // special, dir, fstype
}

impl MountTable {
    pub fn mount(&mut self, special: String, dir: String, fstype: String, _flag: u32) -> isize {
        if self.mnt_list.len() == MNT_MAXLEN {
            return -1;
        }
        if self.mnt_list.iter().find(|&(_, d, _)| *d == dir).is_some() {
            return 0;
        }
        self.mnt_list.push((special, dir, fstype));
        return 0;
    }

    pub fn umount(&mut self, special: String, _flags: u32) -> isize {
        let len = self.mnt_list.len();
        for i in 0..len {
            //println!("[umount] in mntlist = {}", self.mnt_list[i].0);
            if self.mnt_list[i].0 == special || self.mnt_list[i].1 == special {
                self.mnt_list.remove(i);
                return 0;
            }
        }
        return -1;
    }
}

lazy_static! {
    pub static ref MNT_TABLE: Arc<Mutex<MountTable>> = {
        let mnt_table = MountTable {
            mnt_list: Vec::new(),
        };
        Arc::new(Mutex::new(mnt_table))
    };
}
