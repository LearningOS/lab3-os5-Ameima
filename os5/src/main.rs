// 关闭std与main
#![no_std]
#![no_main]
// 启用panic信息与分配错误处理函数
#![feature(panic_info_message)]
#![feature(alloc_error_handler)]

#[macro_use]
extern crate bitflags;
#[macro_use]
extern crate log;

extern crate alloc;

#[macro_use]
mod console;
mod config;
mod lang_items;
mod loader;
mod logging;
mod mm;
mod sbi;
mod sync;
mod syscall;
mod task;
mod timer;
mod trap;

// 将入口点与应用导入一起编译
core::arch::global_asm!(include_str!("entry.asm"));
core::arch::global_asm!(include_str!("link_app.S"));

// 清零bss段
fn clear_bss() {
    extern "C" {
        fn sbss();
        fn ebss();
    }
    unsafe {
        core::slice::from_raw_parts_mut(sbss as usize as *mut u8, ebss as usize - sbss as usize)
            .fill(0);
    }
}

#[no_mangle]
pub fn rust_main() -> ! {
    // 清零bss段
    clear_bss();
    // 开启内核日志
    logging::init();
    // 内核启动
    println!("[kernel] Hello, world!");
    // 初始化内存管理模块
    mm::init();
    // 测试内存管理模块是否正常启动
    mm::remap_test();
    // 初始化任务管理器，启动初始进程
    task::add_initproc();
    // 初始进程启动完毕
    info!("after initproc!");
    // 初始化陷入处理
    trap::init();
    // 接收时钟中断
    trap::enable_timer_interrupt();
    // 设定第一个时钟中断
    timer::set_next_trigger();
    // 列出可执行应用
    loader::list_apps();
    // 进入用户态
    task::run_tasks();
    panic!("Unreachable in rust_main!");
}
