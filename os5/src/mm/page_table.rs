// 定义并实现页表与页表项，被地址空间所维护，主要是给cpu看和使用的

use super::{frame_alloc, FrameTracker, PhysAddr, PhysPageNum, StepByOne, VirtAddr, VirtPageNum};
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use bitflags::*;

bitflags! {
    // 定义页表项标志位
    pub struct PTEFlags: u8 {
        const V = 1 << 0;
        const R = 1 << 1;
        const W = 1 << 2;
        const X = 1 << 3;
        const U = 1 << 4;
        const G = 1 << 5;
        const A = 1 << 6;
        const D = 1 << 7;
    }
}

#[derive(Copy, Clone)]
#[repr(C)]
// 页表项结构体
pub struct PageTableEntry {
    pub bits: usize,
}

// 页表项方法
impl PageTableEntry {
    // 新建页表项，用指定的页帧号与标志位
    pub fn new(ppn: PhysPageNum, flags: PTEFlags) -> Self {
        PageTableEntry {
            bits: ppn.0 << 10 | flags.bits as usize,
        }
    }
    // 新建页表项，但是是空的
    pub fn empty() -> Self {
        PageTableEntry { bits: 0 }
    }
    // 获取页表项中的页帧号
    pub fn ppn(&self) -> PhysPageNum {
        (self.bits >> 10 & ((1usize << 44) - 1)).into()
    }
    // 获取页表项中的标志位
    pub fn flags(&self) -> PTEFlags {
        PTEFlags::from_bits(self.bits as u8).unwrap()
    }
    // 判断页表项是否可用
    pub fn is_valid(&self) -> bool {
        (self.flags() & PTEFlags::V) != PTEFlags::empty()
    }
    // 判断页表项是否可读
    pub fn readable(&self) -> bool {
        (self.flags() & PTEFlags::R) != PTEFlags::empty()
    }
    // 判断页表项是否可写
    pub fn writable(&self) -> bool {
        (self.flags() & PTEFlags::W) != PTEFlags::empty()
    }
    // 判断页表项是否可执行
    pub fn executable(&self) -> bool {
        (self.flags() & PTEFlags::X) != PTEFlags::empty()
    }
}

// 页表结构体
pub struct PageTable {
    root_ppn: PhysPageNum, // 页表本体所在的物理页帧
    frames: Vec<FrameTracker>, // 页表下面挂载的页帧的资源抽象，只挂载页表的页帧
    // 虚拟页的实际的物理页帧不挂载在这里，而是挂载在地址空间的逻辑段的data_frames中
}

// 页表方法
impl PageTable {
    // 新建空页表，会分配一片页帧存储页表，所携带的资源也就页表本身
    pub fn new() -> Self {
        // 分配后是全清零的，这样V标志位也是0
        let frame = frame_alloc().unwrap();
        PageTable {
            root_ppn: frame.ppn,
            frames: vec![frame],
        }
    }
    // 从token新建页表
    pub fn from_token(satp: usize) -> Self {
        Self {
            root_ppn: PhysPageNum::from(satp & ((1usize << 44) - 1)),
            frames: Vec::new(),
        }
    }
    // 在表里找到虚拟页号对应的表项的位置，没有就创建中间的路径
    fn find_pte_create(&mut self, vpn: VirtPageNum) -> Option<&mut PageTableEntry> {
        // 虚拟页号切分成三级
        let mut idxs = vpn.indexes();
        // 从页表根开始找
        let mut ppn = self.root_ppn;
        let mut result: Option<&mut PageTableEntry> = None;
        // 在虚拟页号的每一级中查表
        for (i, idx) in idxs.iter_mut().enumerate() {
            // 取出整个页表的所有页表项，定位到虚拟页号对应的表项位置
            let pte = &mut ppn.get_pte_array()[*idx];
            // 已经到一级页表了，该创建的都创建完了，不管是不是全0的，返回那一项就好
            if i == 2 {
                result = Some(pte);
                break;
            }
            // 页表无效则新建页表
            if !pte.is_valid() {
                // 分配个全0页表
                let frame = frame_alloc().unwrap();
                // 当先表上写上表项，新表项的物理页帧号写进当前表里，并且对应位置V标志位置1
                *pte = PageTableEntry::new(frame.ppn, PTEFlags::V);
                // 新表挂载到当前表里
                self.frames.push(frame);
            }
            // 换下一级
            ppn = pte.ppn();
        }
        result
    }
    // 在表里先找到虚拟页号对应的表项的位置，没有就返回None
    pub fn find_pte(&self, vpn: VirtPageNum) -> Option<&PageTableEntry> {
        let idxs = vpn.indexes();
        let mut ppn = self.root_ppn;
        let mut result: Option<&PageTableEntry> = None;
        for (i, idx) in idxs.iter().enumerate() {
            let pte = &ppn.get_pte_array()[*idx];
            if i == 2 {
                result = Some(pte);
                break;
            }
            // 不同之处在于没有就返回None
            if !pte.is_valid() {
                return None;
            }
            ppn = pte.ppn();
        }
        result
    }
    #[allow(unused)]
    // 在表中添加“虚拟页号->物理页号”的映射，不添加被映射的物理页帧的资源到frame中
    // 物理页帧的资源由地址空间中的逻辑段的data_frames掌管
    pub fn map(&mut self, vpn: VirtPageNum, ppn: PhysPageNum, flags: PTEFlags) {
        // 在表里先找到虚拟页号对应的表项的位置，没有就创建中间的路径
        let pte = self.find_pte_create(vpn).unwrap();
        // 查看找到的位置，如果V是1那就说明已经被映射了，发起报错
        assert!(!pte.is_valid(), "vpn {:?} is mapped before mapping", vpn);
        // V是0表示还没被映射，这样就可以映射了，在表里写入映射信息即可
        *pte = PageTableEntry::new(ppn, flags | PTEFlags::V);
    }
    #[allow(unused)]
    // 在表中解除“虚拟页号->物理页号”的映射，同样不用考虑被映射的页帧的释放问题，那个由地址空间逻辑段掌控
    pub fn unmap(&mut self, vpn: VirtPageNum) {
        // 在表里先找到虚拟页号对应的表项的位置，没有就创建中间的路径
        let pte = self.find_pte_create(vpn).unwrap();
        // 查看找到的位置，如果V是0那就说明还没被映射，发起报错
        assert!(pte.is_valid(), "vpn {:?} is invalid before unmapping", vpn);
        // 清零即可
        *pte = PageTableEntry::empty();
    }
    // 获得虚拟页号对应的物理页号，查表并转换，可能为None
    pub fn translate(&self, vpn: VirtPageNum) -> Option<PageTableEntry> {
        self.find_pte(vpn).copied()
    }
    // 获得虚拟地址对应的物理地址，查表并转换，可能为None
    pub fn translate_va(&self, va: VirtAddr) -> Option<PhysAddr> {
        self.find_pte(va.clone().floor()).map(|pte| {
            //println!("translate_va:va = {:?}", va);
            let aligned_pa: PhysAddr = pte.ppn().into();
            //println!("translate_va:pa_align = {:?}", aligned_pa);
            let offset = va.page_offset();
            let aligned_pa_usize: usize = aligned_pa.into();
            (aligned_pa_usize + offset).into()
        })
    }
    // token化表，方便写入satp
    pub fn token(&self) -> usize {
        8usize << 60 | self.root_ppn.0
    }
}

// 从某用户的地址空间（用token指定）中取出u8缓冲区放在内核堆里供读写，写不会影响用户数据
pub fn translated_byte_buffer(token: usize, ptr: *const u8, len: usize) -> Vec<&'static mut [u8]> {
    let page_table = PageTable::from_token(token);
    let mut start = ptr as usize;
    let end = start + len;
    let mut v = Vec::new();
    while start < end {
        let start_va = VirtAddr::from(start);
        let mut vpn = start_va.floor();
        let ppn = page_table.translate(vpn).unwrap().ppn();
        vpn.step();
        let mut end_va: VirtAddr = vpn.into();
        end_va = end_va.min(VirtAddr::from(end));
        if end_va.page_offset() == 0 {
            v.push(&mut ppn.get_bytes_array()[start_va.page_offset()..]);
        } else {
            v.push(&mut ppn.get_bytes_array()[start_va.page_offset()..end_va.page_offset()]);
        }
        start = end_va.into();
    }
    v
}

// 从某用户的地址空间（用token指定）中取出str放在内核堆里供读写，写不会影响用户数据
pub fn translated_str(token: usize, ptr: *const u8) -> String {
    let page_table = PageTable::from_token(token);
    let mut string = String::new();
    let mut va = ptr as usize;
    loop {
        let ch: u8 = *(page_table
            .translate_va(VirtAddr::from(va))
            .unwrap()
            .get_mut());
        if ch == 0 {
            break;
        } else {
            string.push(ch as char);
            va += 1;
        }
    }
    string
}

// 从某用户的地址空间（用token指定）中取出某种类型数据供直接读写，会影响用户数据
// 调用了get_mut()这个向物理地址写入内容的功能，加上自动查表的抽象，实现了向虚拟地址写入内容的效果
pub fn translated_refmut<T>(token: usize, ptr: *mut T) -> &'static mut T {
    //println!("into translated_refmut!");
    let page_table = PageTable::from_token(token);
    let va = ptr as usize;
    //println!("translated_refmut: before translate_va");
    page_table
        .translate_va(VirtAddr::from(va))
        .unwrap()
        .get_mut()
}
