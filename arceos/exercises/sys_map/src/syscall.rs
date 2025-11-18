#![allow(dead_code)]

use core::ffi::{c_void, c_char, c_int};
use axhal::arch::TrapFrame;
use axhal::trap::{register_trap_handler, SYSCALL};
use axerrno::LinuxError;
use axtask::current;
use axtask::TaskExtRef;
use axhal::paging::MappingFlags;
use arceos_posix_api as api;

const SYS_IOCTL: usize = 29;
const SYS_OPENAT: usize = 56;
const SYS_CLOSE: usize = 57;
const SYS_READ: usize = 63;
const SYS_WRITE: usize = 64;
const SYS_WRITEV: usize = 66;
const SYS_EXIT: usize = 93;
const SYS_EXIT_GROUP: usize = 94;
const SYS_SET_TID_ADDRESS: usize = 96;
const SYS_MMAP: usize = 222;

const AT_FDCWD: i32 = -100;

/// Macro to generate syscall body
///
/// It will receive a function which return Result<_, LinuxError> and convert it to
/// the type which is specified by the caller.
#[macro_export]
macro_rules! syscall_body {
    ($fn: ident, $($stmt: tt)*) => {{
        #[allow(clippy::redundant_closure_call)]
        let res = (|| -> axerrno::LinuxResult<_> { $($stmt)* })();
        match res {
            Ok(_) | Err(axerrno::LinuxError::EAGAIN) => debug!(concat!(stringify!($fn), " => {:?}"),  res),
            Err(_) => info!(concat!(stringify!($fn), " => {:?}"), res),
        }
        match res {
            Ok(v) => v as _,
            Err(e) => {
                -e.code() as _
            }
        }
    }};
}

bitflags::bitflags! {
    #[derive(Debug)]
    /// permissions for sys_mmap
    ///
    /// See <https://github.com/bminor/glibc/blob/master/bits/mman.h>
    struct MmapProt: i32 {
        /// Page can be read.
        const PROT_READ = 1 << 0;
        /// Page can be written.
        const PROT_WRITE = 1 << 1;
        /// Page can be executed.
        const PROT_EXEC = 1 << 2;
    }
}

impl From<MmapProt> for MappingFlags {
    fn from(value: MmapProt) -> Self {
        let mut flags = MappingFlags::USER;
        if value.contains(MmapProt::PROT_READ) {
            flags |= MappingFlags::READ;
        }
        if value.contains(MmapProt::PROT_WRITE) {
            flags |= MappingFlags::WRITE;
        }
        if value.contains(MmapProt::PROT_EXEC) {
            flags |= MappingFlags::EXECUTE;
        }
        flags
    }
}

bitflags::bitflags! {
    #[derive(Debug)]
    /// flags for sys_mmap
    ///
    /// See <https://github.com/bminor/glibc/blob/master/bits/mman.h>
    struct MmapFlags: i32 {
        /// Share changes
        const MAP_SHARED = 1 << 0;
        /// Changes private; copy pages on write.
        const MAP_PRIVATE = 1 << 1;
        /// Map address must be exactly as requested, no matter whether it is available.
        const MAP_FIXED = 1 << 4;
        /// Don't use a file.
        const MAP_ANONYMOUS = 1 << 5;
        /// Don't check for reservations.
        const MAP_NORESERVE = 1 << 14;
        /// Allocation is for a stack.
        const MAP_STACK = 0x20000;
    }
}

#[register_trap_handler(SYSCALL)]
fn handle_syscall(tf: &TrapFrame, syscall_num: usize) -> isize {
    ax_println!("handle_syscall [{}] ...", syscall_num);
    let ret = match syscall_num {
         SYS_IOCTL => sys_ioctl(tf.arg0() as _, tf.arg1() as _, tf.arg2() as _) as _,
        SYS_SET_TID_ADDRESS => sys_set_tid_address(tf.arg0() as _),
        SYS_OPENAT => sys_openat(tf.arg0() as _, tf.arg1() as _, tf.arg2() as _, tf.arg3() as _),
        SYS_CLOSE => sys_close(tf.arg0() as _),
        SYS_READ => sys_read(tf.arg0() as _, tf.arg1() as _, tf.arg2() as _),
        SYS_WRITE => sys_write(tf.arg0() as _, tf.arg1() as _, tf.arg2() as _),
        SYS_WRITEV => sys_writev(tf.arg0() as _, tf.arg1() as _, tf.arg2() as _),
        SYS_EXIT_GROUP => {
            ax_println!("[SYS_EXIT_GROUP]: system is exiting ..");
            axtask::exit(tf.arg0() as _)
        },
        SYS_EXIT => {
            ax_println!("[SYS_EXIT]: system is exiting ..");
            axtask::exit(tf.arg0() as _)
        },
        SYS_MMAP => sys_mmap(
            tf.arg0() as _,
            tf.arg1() as _,
            tf.arg2() as _,
            tf.arg3() as _,
            tf.arg4() as _,
            tf.arg5() as _,
        ),
        _ => {
            ax_println!("Unimplemented syscall: {}", syscall_num);
            -LinuxError::ENOSYS.code() as _
        }
    };
    ret
}

#[allow(unused_variables)]
fn sys_mmap(
    addr: *mut usize,
    length: usize,
    prot: i32,
    flags: i32,
    fd: i32,
    _offset: isize,
) -> isize {
    // unimplemented!("no sys_mmap!");
     return 1;//测试
    // let curr = current();//得到当前进程的task 相当于任务控制块
    // let ext = unsafe { &mut *(curr.task_ext_ptr() as *mut TaskExt) };//获取任务扩展数据
    // let mut space = ext.aspace.lock();//得到任务的地址空间

    // let flags = MmapFlags::from_bits_truncate(flags);//两个标志转换 flags代表映射行为标志 包括修改后对其他进程是否共享等
    // let prot = MmapProt::from_bits_truncate(prot);//prot表示映射的内存权限 是否可读写等
    // let len = align_up_4k(length);

    // // 1. 选地址
    // let vaddr = if flags.contains(MmapFlags::MAP_FIXED) {
    //     VirtAddr::from(addr as usize)//CASE1:强制使用指定地址 无论地址是否可用
    // } else if addr.is_null() {//CASE2：addr.is_null() 代表内核自己决定
    //     space.find_free_area(space.base() + PAGE_SIZE_4K, len,
    //                          AddrRange::new(space.base(), space.end()))?
    // } else {
    //     VirtAddr::from(addr as usize)//CASE3：其他情况 使用提示的地址
    // };

    // // 2. 映射页表
    // space.map_alloc(vaddr, len, MappingFlags::from(prot) | MappingFlags::USER, true)
    //     .map_err(|_| LinuxError::ENOMEM)?;
    //     //在用户地址空间中，建立从虚拟地址 vaddr 开始、长度为 len 的区域与物理页的映射关系
    //     //分配了物理页 并在页表建立了映射

    // // 3. 填充内容
    // if !flags.contains(MmapFlags::MAP_ANONYMOUS) {
    //     let file = get_file_like(fd)?;
    //     let file_size = file.get_size();  // 获取文件大小
        
    //     // 计算实际需要读取的字节数
    //     let read_len = len.min(file_size.saturating_sub(_offset as usize));
        
    //     let mut remain = read_len;
    //     let mut off = _offset as usize;
    //     let mut va = vaddr;
    //     //将虚拟地址转化为内核可访问的多个内核缓冲区 内部是转化为物理地址 再转化为内核虚拟地址
    //     //这样将文件读到这些内核缓冲区
    //     for buf in space.translated_byte_buffer(va, read_len)? {
    //         if remain == 0 { break; }
            
    //         let read = file.read_at(off, buf)? as usize;
    //         if read == 0 { 
    //             // 文件结束但还有剩余空间，填充0或报错？
    //             break; 
    //         }
            
    //         off += read;
    //         remain = remain.saturating_sub(read);
    //     }
        
    //     // 如果是 MAP_PRIVATE，标记为写时复制？
    //     if flags.contains(MmapFlags::MAP_PRIVATE) {
    //         // 可能需要特殊的处理
    //     }
    // }

    // Ok(vaddr.as_usize() as isize)
}

fn sys_openat(dfd: c_int, fname: *const c_char, flags: c_int, mode: api::ctypes::mode_t) -> isize {
    assert_eq!(dfd, AT_FDCWD);
    api::sys_open(fname, flags, mode) as isize
}

fn sys_close(fd: i32) -> isize {
    api::sys_close(fd) as isize
}

fn sys_read(fd: i32, buf: *mut c_void, count: usize) -> isize {
    api::sys_read(fd, buf, count)
}

fn sys_write(fd: i32, buf: *const c_void, count: usize) -> isize {
    api::sys_write(fd, buf, count)
}

fn sys_writev(fd: i32, iov: *const api::ctypes::iovec, iocnt: i32) -> isize {
    unsafe { api::sys_writev(fd, iov, iocnt) }
}

fn sys_set_tid_address(tid_ptd: *const i32) -> isize {
    let curr = current();
    curr.task_ext().set_clear_child_tid(tid_ptd as _);
    curr.id().as_u64() as isize
}

fn sys_ioctl(_fd: i32, _op: usize, _argp: *mut c_void) -> i32 {
    ax_println!("Ignore SYS_IOCTL");
    0
}
