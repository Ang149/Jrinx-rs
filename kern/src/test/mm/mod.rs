pub(super) mod phys {
    use core::mem::forget;

    use alloc::sync::Arc;
    use jrinx_addr::PhysAddr;
    use jrinx_error::Result;
    use jrinx_phys_frame::PhysFrame;
    use jrinx_testdef::testdef;

    #[testdef]
    fn test() {
        let (frame1, addr1) = alloc().unwrap();
        let (frame2, addr2) = alloc().unwrap();

        assert_ne!(addr1, addr2);

        let frame3 = frame1.clone();
        drop(frame1);
        drop(frame2);

        while let Ok((frame, _)) = alloc() {
            forget(frame);
        }
        drop(frame3);

        let (_, addr4) = alloc().unwrap();
        assert_eq!(addr1, addr4);
    }

    fn alloc() -> Result<(Arc<PhysFrame>, PhysAddr)> {
        let f = PhysFrame::alloc()?;
        let a = f.addr();
        Ok((f, a))
    }
}

pub(super) mod virt {
    use core::mem;

    use jrinx_addr::VirtAddr;
    use jrinx_hal::{Hal, Vm};
    use jrinx_paging::{GenericPagePerm, GenericPageTable, PagePerm};
    use jrinx_phys_frame::PhysFrame;
    use jrinx_testdef::testdef;
    use jrinx_vmm::KERN_PAGE_TABLE;
    use rand::{rngs::SmallRng, RngCore, SeedableRng};

    #[testdef]
    fn test() {
        let mut rng =
            SmallRng::seed_from_u64(option_env!("RAND_SEED").unwrap_or("0").parse().unwrap());

        let vaddr1 = VirtAddr::new(jrinx_config::PAGE_SIZE);
        let vaddr2 = VirtAddr::new(jrinx_config::PAGE_SIZE * 2);

        for _ in 0..10 {
            let mut page_table = KERN_PAGE_TABLE.write();
            let frame = PhysFrame::alloc().unwrap();
            let paddr = frame.addr();
            page_table
                .map(vaddr1, frame, PagePerm::G | PagePerm::W | PagePerm::R)
                .unwrap();
            let (frame, perm) = page_table.lookup(vaddr1).unwrap();

            assert_eq!(frame.addr(), paddr);
            page_table.map(vaddr2, frame, perm).unwrap();

            let (paddr1, perm1) = page_table.translate(vaddr1).unwrap();
            let (paddr2, perm2) = page_table.translate(vaddr2).unwrap();
            assert_eq!(paddr1, paddr2);
            assert_eq!(perm1.bits(), perm2.bits());

            hal!().vm().sync_all();

            let space = [
                vaddr1.as_usize() as *mut u64,
                paddr.to_virt().as_usize() as *mut u64,
            ];

            for i in 0..jrinx_config::PAGE_SIZE / mem::size_of::<usize>() {
                let src = space[i % 2];
                let dst = space[1 - i % 2];
                let rand = rng.next_u64();
                write(src, rand);
                assert_eq!(read(dst), rand);
                write(src, !read(dst));
                assert_eq!(read(src), read(dst));
            }
        }

        let mut page_table = KERN_PAGE_TABLE.write();
        page_table.unmap(vaddr1).unwrap();
        page_table.unmap(vaddr2).unwrap();
    }

    fn write<T>(src: *mut T, val: T)
    where
        T: Clone + Copy,
    {
        unsafe { *src = val }
    }

    fn read<T>(dst: *const T) -> T
    where
        T: Clone + Copy,
    {
        unsafe { *dst }
    }
}
