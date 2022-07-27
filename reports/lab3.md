# lab3实验报告
## 实现功能

首先实现了spawn系统调用，采用new一个任务控制块，然后手动添加父子关系的方式实现。
接着实现了stride调度算法。首先在任务控制块中新加入代表长度的task_pass字段和代表优先级的task_prioriy字段。
然后让在创建进程的时候初始化它们，在调度进程的时候根据优先级task_prioriy修改task_pass。
接着更新了任务调度器的fetch函数，使它扫一遍任务队列后挑出pass最短的返回。最后实现了设置优先级的系统调用。

## 简答作业
stride 算法原理非常简单，但是有一个比较大的问题。
例如两个 stride = 10 的进程，使用 8bit 无符号整形储存 pass， 
p1.pass = 255, p2.pass = 250，在 p2 执行一个时间片后，理论上下一次应该 p1 执行。

* 实际情况是轮到 p1 执行吗？为什么？

    __答：__ 不是。使用8bit无符号整形储存pass，p2执行一个时间片后pass加上stride也就是10，将会溢出变成5，比p1小所以依旧p2。

我们之前要求进程优先级 >= 2 其实就是为了解决这个问题。可以证明， 在不考虑溢出的情况下 , 
在进程优先级全部 >= 2 的情况下，如果严格按照算法执行，那么 PASS_MAX – PASS_MIN <= BigStride / 2。

* 为什么？尝试简单说明（不要求严格证明）。

    __答：__ 调度器每次都会调用pass值最小的进程，最大pass和最小pass之间相差不会超过一个最大步长。

* 已知以上结论，考虑溢出的情况下，可以为 pass 设计特别的比较器，让 BinaryHeap<Pass> 的 pop 方法能返回真正最小的 Pass。
补全下列代码中的 partial_cmp 函数，假设两个 Pass 永远不会相等。

```rust
use core::cmp::Ordering;

struct Pass(u64);

impl PartialOrd for Pass {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some((self.0 % (core::u64::MAX / 2)).cmp(&(other.0 % (core::u64::MAX / 2))))
    }
}

impl PartialEq for Pass {
    fn eq(&self, other: &Self) -> bool {
        false
    }
}
```
