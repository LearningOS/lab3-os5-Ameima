// Trap上下文的实现，目前Trap上下文是放在各个用户地址空间的一个特定位置上

use riscv::register::sstatus::{self, Sstatus, SPP};

#[repr(C)]
// 保存的Trap上下文结构，与汇编中的压栈顺序一致
pub struct TrapContext {
    // 32个通用寄存器
    pub x: [usize; 32],
    /// sstatus给出 Trap 发生之前 CPU 处在哪个特权级
    pub sstatus: Sstatus,
    /// sepc记录 Trap 发生之前执行的最后一条指令的地址
    pub sepc: usize,

    // 下面kernel_satp和trap_handler初始化后保持不变
    // 内核页表token，用于切换到内核态时切换地址空间
    pub kernel_satp: usize,
    // 当前进程的内核栈指针位置，取代了之前sstatus的功能，现在sstatus仅作为周转
    pub kernel_sp: usize,
    // trap处理入口
    pub trap_handler: usize,
}

// Trap上下文方法，可用用于初次进入进程时构建挂起的Trap恢复快照
impl TrapContext {
    // 设置快照中的栈顶指针，主要是给下面的方法使用
    pub fn set_sp(&mut self, sp: usize) {
        self.x[2] = sp;
    }
    // 构建初次进入进程时构建挂起的Trap恢复快照，被elf读取函数按照elf中的预期进行设置，返回一个Trap上下文
    pub fn app_init_context(
        entry: usize, // 应用入口点
        sp: usize, // 应用栈指针
        kernel_satp: usize, // 内核页表token
        kernel_sp: usize, // 当前进程的内核栈指针位置
        trap_handler: usize, // trap处理入口
    ) -> Self {
        let mut sstatus = sstatus::read();
        // 设置恢复到用户态
        sstatus.set_spp(SPP::User);
        // 由于还没运行，所以大部分寄存器设置空的就好
        let mut cx = Self {
            x: [0; 32],
            sstatus,
            sepc: entry,
            kernel_satp,
            kernel_sp,
            trap_handler,
        };
        // 栈指针要设置
        cx.set_sp(sp);
        cx
    }
}
