// 处理特权级切换的陷入模块

mod context;

use crate::config::{TRAMPOLINE, TRAP_CONTEXT};
use crate::syscall::syscall;
use crate::task::{
    current_trap_cx, current_user_token, exit_current_and_run_next, suspend_current_and_run_next,
};
use crate::timer::set_next_trigger;
use riscv::register::{
    mtvec::TrapMode,
    scause::{self, Exception, Interrupt, Trap},
    sie, stval, stvec,
};

// 陷入要用到的汇编，放进来一起编译，实际上会被作为跳板段布置到各个新建的地址空间的最头部
core::arch::global_asm!(include_str!("trap.S"));

// 初始化Trap处理，因为最开始是在内核中，所以设置为内核的Trap处理入口（指直接panic）
pub fn init() {
    set_kernel_trap_entry();
}

// 设置内核态的Trap处理入口，stvec寄存器的作用就是用来存Trap处理函数入口的
fn set_kernel_trap_entry() {
    unsafe {
        // 用这个会panic的函数处理内核的trap（定义在最下面）
        stvec::write(trap_from_kernel as usize, TrapMode::Direct);
    }
}

// 设置用户态的Trap入口，就是跳板页（也就是每个地址空间最顶端的一页）
fn set_user_trap_entry() {
    unsafe {
        stvec::write(TRAMPOLINE as usize, TrapMode::Direct);
    }
}

// 允许接收时钟中断
pub fn enable_timer_interrupt() {
    unsafe {
        sie::set_stimer();
    }
}

#[no_mangle]
// 跳板页保存完Trap上下文后，最终会跳到这里进行Trap处理
pub fn trap_handler() -> ! {
    // 进了内核态就换内核态的Trap处理函数
    set_kernel_trap_entry();
    // 读取Trap原因
    let scause = scause::read();
    // 读取Trap前运行至的地址
    let stval = stval::read();
    // 分发处理
    match scause.cause() {
        // 系统调用
        Trap::Exception(Exception::UserEnvCall) => {
            // 获取Trap上下文
            let mut cx = current_trap_cx();
            // 让中断位置指针向前步进1个指令，表示这个调用已经受理了
            cx.sepc += 4;
            // 跳转到对应的系统调用处理函数
            let result = syscall(cx.x[17], [cx.x[10], cx.x[11], cx.x[12]]); // 任务切换的快照恢复后总是会出现在这里（略去里面那层系统调用的函数的话），
            // 通过switch修改了ra导致ret时回到了另一个进程的这个断点，
            // 同时旧的进程也是因为能够跳回这里的ra被快照后修改了所以才跳到另一个进程去了，现在还是在内核态所以保存的也是内核栈sp
            
            // 系统调用可能会修改上下文，重新获取
            cx = current_trap_cx();
            // 给出结果（0或-1）
            cx.x[10] = result as usize;
        }
        // 异常
        Trap::Exception(Exception::StoreFault)
        | Trap::Exception(Exception::StorePageFault)
        | Trap::Exception(Exception::InstructionFault)
        | Trap::Exception(Exception::InstructionPageFault)
        | Trap::Exception(Exception::LoadFault)
        | Trap::Exception(Exception::LoadPageFault) => {
            // 打印错误信息
            println!(
                "[kernel] {:?} in application, bad addr = {:#x}, bad instruction = {:#x}, core dumped.",
                scause.cause(),
                stval,
                current_trap_cx().sepc,
            );
            // 杀死进程，给出退出码
            exit_current_and_run_next(-2);
        }
        // 无效指令
        Trap::Exception(Exception::IllegalInstruction) => {
            println!("[kernel] IllegalInstruction in application, core dumped.");
            // 杀死进程，给出退出码
            exit_current_and_run_next(-3);
        }
        // 时钟中断
        Trap::Interrupt(Interrupt::SupervisorTimer) => {
            set_next_trigger(); // 设置新的时钟中断
            // 挂起进程
            suspend_current_and_run_next();
        }
        // 未知错误
        _ => {
            panic!(
                "Unsupported trap {:?}, stval = {:#x}!",
                scause.cause(),
                stval
            );
        }
    }
    // 处理完trap后从Trap返回
    trap_return();
}

#[no_mangle]
// 处理完Trap后的返回
pub fn trap_return() -> ! {
    // 要进入用户态了，把Trap处理函数换回去
    set_user_trap_entry();
    // trap上下文在每个用户的地址空间中都是同一个固定位置，
    // 所以在汇编里也没必要保存了，每次通过a0传给它就行了
    let trap_cx_ptr = TRAP_CONTEXT;
    // 用户页表的位置
    let user_satp = current_user_token();
    // 导入符号
    extern "C" {
        fn __alltraps();
        fn __restore();
    }
    // 计算 __restore 虚地址，因为两个标签都是放在text段中的地址不是后来被放到顶上的那个虚地址，所以要计算
    let restore_va = __restore as usize - __alltraps as usize + TRAMPOLINE;
    unsafe {
        core::arch::asm!(
            "fence.i", // 清缓存，
            // 因为在内核中进行的一些操作可能导致一些原先存放某个应用代码的物理页帧
            // 如今用来存放数据或者是其他应用的代码，
            // i-cache 中可能还保存着该物理页帧的错误快照。
            "jr {restore_va}", // 跳转到restore_va
            restore_va = in(reg) restore_va,
            in("a0") trap_cx_ptr, // 和汇编中照应。a0放Trap上下文位置，a1放用户页表token
            in("a1") user_satp,
            options(noreturn)
        );
    }
}

#[no_mangle]
// 内核态遇到Trap直接panic
pub fn trap_from_kernel() -> ! {
    panic!("a trap {:?} from kernel!", scause::read().cause());
}

pub use context::TrapContext;
