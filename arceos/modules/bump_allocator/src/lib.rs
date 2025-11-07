#![no_std]

use allocator::{BaseAllocator, ByteAllocator, PageAllocator};
use core::ptr::NonNull;
//这个allocator是外部库 在cargo.toml中有
//它定义了这三个BaseAllocator, ByteAllocator, PageAllocator trait
//我只要考虑具体实现它们就好


/// Early memory allocator
/// Use it before formal bytes-allocator and pages-allocator can work!
/// This is a double-end memory range:
/// - Alloc bytes forward
/// - Alloc pages backward
///
/// [ bytes-used | avail-area | pages-used ]
/// |            | -->    <-- |            |
/// start       b_pos        p_pos       end
///
/// For bytes area, 'count' records number of allocations.
/// When it goes down to ZERO, free bytes-used area.
/// For pages area, it will never be freed!
///
pub struct EarlyAllocator<const SIZE: usize> {
    start:usize,//内存起始地址
    b_pos:usize,
    p_pos:usize,
    end:usize,//内存结束地址
    count:usize,//记录字节分配的次数 
    //由于bump分配器不好归还分配区域 所以使用count来帮助
}

impl<const SIZE: usize> EarlyAllocator<SIZE> {// 创建未初始化的分配器
    pub const fn new() -> Self {
        Self {
            start:0,
            b_pos:0,
            p_pos:0,
            end:0,
            count:0,
        }
    }
}

impl<const SIZE: usize> BaseAllocator for EarlyAllocator<SIZE> {
    fn init(&mut self, start: usize, size: usize) {//真正的初始化 运行时设置内存区域
        self.start=start;
        self.b_pos=start;
        self.p_pos=start+size;
        self.end=start+size;
        self.count=0;
    }

    fn add_memory(&mut self, start: usize, size: usize) -> allocator::AllocResult {
        //扩展新的内存空间 但bump算法本身不好支持扩展
        todo!()
    }
}

impl<const SIZE: usize> ByteAllocator for EarlyAllocator<SIZE> {
    fn alloc(
        &mut self,
        layout: core::alloc::Layout,//layout是rust标准库里定义的结构体 
        //  两个成员size: usize, // 需要分配的字节数    align: usize,     // 内存对齐要求（必须是2的幂）
    ) -> allocator::AllocResult<core::ptr::NonNull<u8>> {
        //AllocResult<NonNull<u8>>  是Result<NonNull<u8>, AllocError>的别名
        //成功返回一个nonnull非空指针 指向u8的   失败返回allocerror类型
        let align=layout.align();
        let size=layout.size();//两个量是私有的 通过方法访问
        //分配的内存起始地址要是align的倍数
        let aligned_addr = (self.b_pos + align - 1) & !(align - 1);//经典的对齐公式  !(align - 1) 创造了掩码 与之前结果按位与 将低位置零
        self.b_pos+=size;
        self.count+=1;
        Ok(unsafe { NonNull::new_unchecked(aligned_addr as *mut u8) })

    }

    fn dealloc(&mut self, pos: core::ptr::NonNull<u8>, layout: core::alloc::Layout) {
        //当dealloc使得 count变0时 再释放字节分配区域
        self.count-=1;
        if self.count==0 {
            // println!("[BUMP]重置字节区域 b_pos:{}--->{}",self.b_pos,self.start);
            self.b_pos=self.start;
        }
    }

    fn total_bytes(&self) -> usize {//总内存大小
        self.end-self.start
    }

    fn used_bytes(&self) -> usize {//已使用字节
        self.b_pos-self.start
    }

    fn available_bytes(&self) -> usize {//可用字节  即还能分配多少字节
        self.p_pos-self.b_pos
    }
}

impl<const SIZE: usize> PageAllocator for EarlyAllocator<SIZE> {
    const PAGE_SIZE: usize = SIZE;
    //在 Rust 中，trait 里定义的常量是关联常量 需要通过类型来访问 Self::PAGE_SIZE
    fn alloc_pages(
        &mut self,
        num_pages: usize,
        align_pow2: usize,
        //align_pow2 表示对齐值的2的幂指数，而不是对齐值本身
    ) -> allocator::AllocResult<usize> {
        // 使用 align_pow2 可以直接进行位运算
        let align = 1 << align_pow2;  // 快速计算对齐值
        //计算边界
        let aligned_addr = (self.p_pos - align + 1) & !(align - 1);

        //更新p_pos
        self.p_pos=aligned_addr-num_pages*Self::PAGE_SIZE;

        Ok(aligned_addr)
    }

    fn dealloc_pages(&mut self, pos: usize, num_pages: usize) {//页分配区域不会free
        todo!()
    }

    fn total_pages(&self) -> usize {
        (self.end-self.start)/Self::PAGE_SIZE
    }

    fn used_pages(&self) -> usize {
        (self.end-self.p_pos)/Self::PAGE_SIZE
    }

    fn available_pages(&self) -> usize {
        (self.p_pos-self.b_pos)/Self::PAGE_SIZE
    }
}