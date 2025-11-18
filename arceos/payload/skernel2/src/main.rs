#![no_std]
#![no_main]

use core::panic::PanicInfo;

#[no_mangle]
unsafe extern "C" fn _start() -> ! {//!代表永远不会返回
    core::arch::asm!(
        "csrr a1, mhartid",//读cpu核心id的寄存器 VS-mode 通常不能直接读取 mhartid cpu检测到违规时会自动VM-EXIT
        "ld a0, 64(zero)",//从地址 0x40 (64) 加载数据到 a0 寄存器
        //大多数系统中：地址 0x0-0xFFF 通常是未映射区域或受保护区域
        //ld rd, offset(rs1)  # 从地址 [rs1 + offset] 加载到 rd
        //ld 指令必须要有基地址寄存器 所以不能直接ld a0 64
        "li a7, 8",
        "ecall",
        options(noreturn) // 告诉Rust这个asm不会返回
    )
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
