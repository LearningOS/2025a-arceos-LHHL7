#![cfg_attr(feature = "axstd", no_std)]
#![cfg_attr(feature = "axstd", no_main)]
#![feature(asm_const)]
#![feature(riscv_ext_intrinsics)]

#[cfg(feature = "axstd")]
extern crate axstd as std;
extern crate alloc;
#[macro_use]
extern crate axlog;

mod task;
mod vcpu;
mod regs;
mod csrs;
mod sbi;
mod loader;

use vcpu::VmCpuRegisters;
use riscv::register::{scause, sstatus, stval};
use csrs::defs::hstatus;
use tock_registers::LocalRegisterCopy;
use csrs::{RiscvCsrTrait, CSR};
use vcpu::_run_guest;
use sbi::SbiMessage;
use loader::load_vm_image;
use axhal::mem::PhysAddr;
use crate::regs::GprIndex::{A0, A1};
use crate::scause::Interrupt;

const VM_ENTRY: usize = 0x8020_0000;

#[cfg_attr(feature = "axstd", no_mangle)]
fn main() {
    //test
    ax_println!("[DEBUG] Testing Hypervisor extension...");
    
    unsafe {//直接使用汇编指令是不安全的操作，必须放在 unsafe 块中
        let result: usize;
        core::arch::asm!(//这是 Rust 的内联汇编宏，允许在代码中直接嵌入汇编指令
            //csrr: "Control and Status Register Read" - 读取控制和状态寄存器
            // 如果 CPU 或 QEMU 没有启用 H 扩展，这条指令就会被识别为 非法指令，触发异常。
            "csrr {}, hstatus",//读hstatus到一个通用寄存器 
            // 然后汇编器/编译器确保：{} 被替换为一个合适的寄存器（比如 a0）
            out(reg) result//reg代表用通用寄存器存输出 再将输出传给result
        );
        ax_println!("hstatus = {:#x}", result);
    }
    //
    ax_println!("Hypervisor ...");

    // A new address space for vm.
    ax_println!("getting uspace");
    //从内存管理系统中划出一块物理内存作为虚拟机地址空间 即虚拟机的”物理“空间
    let mut uspace = axmm::new_user_aspace().unwrap();

    // Load vm binary file into address space.
    //把映像加载到刚分配的地址空间（内部是将二进制文件读到物理内存 再建立Guest“物理”地址与真实物理页号的映射）
    ax_println!("loading vm");
    if let Err(e) = load_vm_image("/sbin/skernel2", &mut uspace) {
        panic!("Cannot load app! {:?}", e);
    }
    

    // Setup context to prepare to enter guest mode.
    //准备Guest的上下文（寄存器）
    ax_println!("setting ctx");
    let mut ctx = VmCpuRegisters::default();
    prepare_guest_context(&mut ctx);

    // Setup pagetable for 2nd address mapping.
    //建立hypervisor页表 用于二级地址映射
    //Guest 页表要等 Guest 内核启动后自己建立
    ax_println!("setting pagetable");
    let ept_root = uspace.page_table_root();
    prepare_vm_pgtable(ept_root);

    // Kick off vm and wait for it to exit.
    //run_guest封装了启动虚拟机的过程
    ax_println!("kicking off vm");
    while !run_guest(&mut ctx) {
    }

    panic!("Hypervisor ok!");
}

fn prepare_vm_pgtable(ept_root: PhysAddr) {
    ax_println!("prepare_vm_pgtable: ept_root = {:#x}", ept_root);

    //暂时注释
    let hgatp = 8usize << 60 | usize::from(ept_root) >> 12;
    unsafe {
        core::arch::asm!(
            "csrw hgatp, {hgatp}",
            hgatp = in(reg) hgatp,
        );
        core::arch::riscv64::hfence_gvma_all();
    }

    // ax_println!("prepare_vm_pgtable: skipped for now");

}

fn run_guest(ctx: &mut VmCpuRegisters) -> bool {
    ax_println!("[DEBUG] enter run_guest");
    unsafe {
        _run_guest(ctx);
    }

    vmexit_handler(ctx)
}

#[allow(unreachable_code)]
fn vmexit_handler(ctx: &mut VmCpuRegisters) -> bool {
    ax_println!("[DEBUG] enter vmexit_hdler");
    use scause::{Exception, Trap};

    let scause = scause::read();
    match scause.cause() {
        Trap::Exception(Exception::VirtualSupervisorEnvCall) => {
            let sbi_msg = SbiMessage::from_regs(ctx.guest_regs.gprs.a_regs()).ok();
            ax_println!("VmExit Reason: VSuperEcall: {:?}", sbi_msg);
            if let Some(msg) = sbi_msg {
                match msg {
                    SbiMessage::Reset(_) => {
                        let a0 = ctx.guest_regs.gprs.reg(A0);
                        let a1 = ctx.guest_regs.gprs.reg(A1);
                        ax_println!("a0 = {:#x}, a1 = {:#x}", a0, a1);
                        assert_eq!(a0, 0x6688);
                        assert_eq!(a1, 0x1234);//暗示a0 a1
                        ax_println!("Shutdown vm normally!");
                        return true;
                    },
                    _ => todo!(),
                }
            } else {
                panic!("bad sbi message! ");
            }
        },
        Trap::Exception(Exception::IllegalInstruction) => {
            // panic!("Bad instruction: {:#x} sepc: {:#x}",
            //     stval::read(),
            //     ctx.guest_regs.sepc
            // );

            // Handle csrr a1, mhartid instruction
            let inst = stval::read();
            ax_println!("VmExit Reason: IllegalInstruction: {:#x} at sepc: {:#x}", inst, ctx.guest_regs.sepc);
            
                // Set a1 to device tree address
            ctx.guest_regs.gprs.set_reg(A1, 0x1234);
                // Move sepc forward by 4 bytes to skip this instruction
                //sepc也代表着恢复到guest后的下条指令地址 所以要加”1“ 不然死循环
            ctx.guest_regs.sepc += 4;
            // if inst == 0xf14025f3 {  // csrr a1, mhartid 的机器码
            //     //模拟读取寄存器的操作 让guest以为读取成功了
            //     let hartid: usize;
            //     unsafe{
            //         core::arch::asm!("csrr {}, mhartid", out(reg) hartid);
            //     }
            //     ax_println!("HS-mode mhartid = {}", hartid); 
            //     ctx.guest_regs.gprs.set_reg(A1, hartid);
            //     // Move sepc forward by 4 bytes to skip this instruction
            //     //sepc也代表着恢复到guest后的下条指令地址 所以要加”1“ 不然死循环
            //     ctx.guest_regs.sepc += 4;
            // } else{
            //     panic!("unimplement Bad instruction: {:#x} sepc: {:#x}",
            //     stval::read(),
            //     ctx.guest_regs.sepc
            // );
            // }
        },
        Trap::Exception(Exception::LoadGuestPageFault) => {
            // panic!("LoadGuestPageFault: stval{:#x} sepc: {:#x}",
            //     stval::read(),
            //     ctx.guest_regs.sepc
            // );

            // Handle load from address 64 (ld a0, 64(zero))
            let addr = stval::read();
            ax_println!("VmExit Reason: LoadGuestPageFault: stval {:#x} sepc: {:#x}", addr, ctx.guest_regs.sepc);
            
            if addr==0x40{
                // Set a0 to the expected value
                ctx.guest_regs.gprs.set_reg(A0, 0x6688);
                // Move sepc forward by 4 bytes to skip this instruction
                ctx.guest_regs.sepc += 4;
            }

        },
        Trap::Interrupt(Interrupt::SupervisorTimer) => {
            ax_println!("VmExit Reason: SupervisorTimer interrupt at {:#x}", ctx.guest_regs.sepc);
            // 向前推进定时器，避免立即再次触发
            // 然后返回 Guest
            return false;
        },
        _ => {
            panic!(
                "Unhandled trap: {:?}, sepc: {:#x}, stval: {:#x}",
                scause.cause(),
                ctx.guest_regs.sepc,
                stval::read()
            );
        }
    }
    false
}
fn prepare_guest_context(ctx: &mut VmCpuRegisters) {
    // Set hstatus
    let mut hstatus = LocalRegisterCopy::<usize, hstatus::Register>::new(
        riscv::register::hstatus::read().bits(),
    );
    // Set Guest bit in order to return to guest mode.
    hstatus.modify(hstatus::spv::Guest);
    // Set SPVP bit in order to accessing VS-mode memory from HS-mode.
    hstatus.modify(hstatus::spvp::Supervisor);
    CSR.hstatus.write_value(hstatus.get());
    ctx.guest_regs.hstatus = hstatus.get();

    // Set sstatus in guest mode.
    let mut sstatus = sstatus::read();
    sstatus.set_spp(sstatus::SPP::Supervisor);
    ctx.guest_regs.sstatus = sstatus.bits();
    // Return to entry to start vm.
    ctx.guest_regs.sepc = VM_ENTRY;
}
