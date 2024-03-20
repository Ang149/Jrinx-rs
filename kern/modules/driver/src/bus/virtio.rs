use core::{
    alloc::{Allocator, Layout},
    ptr::NonNull,
};

use alloc::alloc::Global;
use jrinx_config::{PAGE_SIZE, REMAP_MEM_OFFSET};
use virtio_drivers::Hal;

pub struct VirtioHal;
unsafe impl Hal for VirtioHal {
    fn dma_alloc(
        pages: usize,
        direction: virtio_drivers::BufferDirection,
    ) -> (virtio_drivers::PhysAddr, core::ptr::NonNull<u8>) {
        let layout = Layout::from_size_align(pages * PAGE_SIZE, PAGE_SIZE).unwrap();
        let ptr = Global.allocate_zeroed(layout);
        let ptr = ptr.unwrap();
        let addr = ptr.cast::<usize>().as_ptr() as usize;
        // unsafe{
        //     for i in 0..pages{
        //         BootPageTable.map(
        //             VirtAddr::new(addr + i * PAGE_SIZE),
        //             PhysAddr::new(addr - REMAP_MEM_OFFSET  + i * PAGE_SIZE),
        //         );
        //     }
        // }
        (addr - REMAP_MEM_OFFSET, ptr.cast::<u8>())
    }

    unsafe fn dma_dealloc(
        paddr: virtio_drivers::PhysAddr,
        vaddr: core::ptr::NonNull<u8>,
        pages: usize,
    ) -> i32 {
        let layout = Layout::from_size_align(pages * PAGE_SIZE, PAGE_SIZE).unwrap();
        Global.deallocate(vaddr, layout);
        0
    }
    unsafe fn mmio_phys_to_virt(
        paddr: virtio_drivers::PhysAddr,
        size: usize,
    ) -> core::ptr::NonNull<u8> {
        NonNull::new((paddr + REMAP_MEM_OFFSET) as *mut _).unwrap()
    }

    unsafe fn share(
        buffer: core::ptr::NonNull<[u8]>,
        direction: virtio_drivers::BufferDirection,
    ) -> virtio_drivers::PhysAddr {
        let vaddr = buffer.as_ptr() as *mut u8 as usize;
        vaddr - REMAP_MEM_OFFSET
    }

    unsafe fn unshare(
        paddr: virtio_drivers::PhysAddr,
        buffer: core::ptr::NonNull<[u8]>,
        direction: virtio_drivers::BufferDirection,
    ) {
    }
}
