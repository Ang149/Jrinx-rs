use core::fmt::Display;

use alloc::string::String;
use bitflags::bitflags;

use crate::{
    conf,
    mm::{
        phys::PhysAddr,
        virt::{PageTable, VirtAddr},
    },
};

bitflags! {
    #[derive(Clone, Copy)]
    pub struct PagePerm: usize {
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

impl Display for PagePerm {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut s = String::from("VRWXUG");
        for i in 0..6 {
            if self.bits() & (1 << i) == 0 {
                s.replace_range(i..=i, "-");
            }
        }
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct PageTableEntry {
    bits: usize,
}

impl PageTableEntry {
    pub fn new(phys_addr: PhysAddr, perm: PagePerm) -> Self {
        Self {
            bits: (phys_addr.align_page_down().as_usize() >> 2) | perm.bits(),
        }
    }

    pub fn as_tuple(self) -> (PhysAddr, PagePerm) {
        let phys_addr = PhysAddr::new((self.bits << 2) & !(conf::PAGE_SIZE - 1));
        let perm = PagePerm::from_bits_truncate(self.bits);
        (phys_addr, perm)
    }

    pub fn is_valid(self) -> bool {
        self.bits & PagePerm::V.bits() != 0
    }
}

pub fn enable_pt_mapping(page_table: &PageTable) {
    let pt_ppn = page_table.addr().as_usize() / conf::PAGE_SIZE;
    unsafe {
        #[cfg(feature = "pt_level_3")]
        riscv::register::satp::set(riscv::register::satp::Mode::Sv39, 0, pt_ppn);
        riscv::asm::sfence_vma_all();
    }
}

pub fn sync(asid: usize, addr: VirtAddr) {
    unsafe {
        riscv::asm::sfence_vma(asid, addr.as_usize());
    }
}