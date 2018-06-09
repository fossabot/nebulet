use x86_64::structures::paging::{PageTable, PageTableFlags, RecursivePageTable,
    PhysFrame, Page, Mapper, Size4KB, MapperFlush, 
    MapToError, UnmapError, FlagUpdateError};
use x86_64::ux::u9;

use arch::lock::{IrqLock, IrqGuard};
use arch::memory;
use core::cell::UnsafeCell;

const P4: *mut PageTable = 0xffffffff_fffff000 as *mut _;
const RECURSIVE_PAGE_INDEX: u9 = u9::MAX;
static PAGE_TABLE_LOCK: IrqLock<Option<RecursivePageTable>> = IrqLock::new(None);

pub unsafe fn init() -> PageMapper {
    *PAGE_TABLE_LOCK.lock() = Some(RecursivePageTable::new_unchecked(&mut*P4, RECURSIVE_PAGE_INDEX));
    PageMapper::new()
}

pub struct PageMapper {
    table: IrqGuard<'static, RecursivePageTable<'static>>,
}

impl PageMapper {
    pub unsafe fn new() -> Self {
        let table = PAGE_TABLE_LOCK.lock_map(|opt| opt.as_mut().unwrap());
        PageMapper {
            table,
        }
    }

    pub fn map(&mut self, page: Page<Size4KB>, flags: PageTableFlags) -> Result<MapperFlush<Size4KB>, MapToError> {
        let mut frame_allocator = || memory::allocate_frame();
        let frame = frame_allocator()
            .expect("Couldn't allocate any frames!");
        self.table.map_to(page, frame, flags, &mut frame_allocator)
    }

    pub fn map_to(&mut self, page: Page<Size4KB>, frame: PhysFrame<Size4KB>, flags: PageTableFlags) -> Result<MapperFlush<Size4KB>, MapToError> {
        let mut frame_allocator = || memory::allocate_frame();
        self.table.map_to(page, frame, flags, &mut frame_allocator)
    }

    pub fn unmap(&mut self, page: Page<Size4KB>) -> Result<MapperFlush<Size4KB>, UnmapError> {
        let mut frame_deallocator = |frame| memory::deallocate_frame(frame);
        self.table.unmap(page, &mut frame_deallocator)
    }

    pub fn remap(&mut self, page: Page<Size4KB>, flags: PageTableFlags) -> Result<MapperFlush<Size4KB>, FlagUpdateError> {
        self.table.update_flags(page, flags)
    }

    pub fn translate(&mut self, page: Page<Size4KB>) -> Option<PhysFrame> {
        self.table.translate(page)
    }
    
    /// For faster mapping of a group of frames
    pub fn lock<'table>(&'table mut self) -> LockedPageMapper<'table, 'static, impl memory::FrameAllocator> {
        let fa = memory::FRAME_ALLOCATOR.lock_map(|opt| opt.as_mut().unwrap());
        LockedPageMapper {
            table: UnsafeCell::new(&mut self.table),
            allocator_guard: UnsafeCell::new(fa),
        }
    }
}

pub struct LockedPageMapper<'table, 'allocator, FA: 'allocator + memory::FrameAllocator> {
    table: UnsafeCell<&'table mut RecursivePageTable<'static>>,
    allocator_guard: UnsafeCell<IrqGuard<'allocator, FA>>, 
}

impl<'table, 'allocator, FA: memory::FrameAllocator> LockedPageMapper<'table, 'allocator, FA> {
    fn table(&self) -> &mut RecursivePageTable<'static> {
        unsafe { &mut *self.table.get() }
    }

    fn allocator(&self) -> &mut FA {
        unsafe { &mut *self.allocator_guard.get() }
    }

    pub fn map(&mut self, page: Page<Size4KB>, flags: PageTableFlags) -> Result<MapperFlush<Size4KB>, MapToError> {
        let allocator = self.allocator();
        let mut frame_allocator = || allocator.allocate_frame();

        let frame = frame_allocator()
            .unwrap();
        
        self.table().map_to(page, frame, flags, &mut frame_allocator)
    }

    pub fn unmap(&mut self, page: Page<Size4KB>) -> Result<MapperFlush<Size4KB>, UnmapError> {
        let allocator = self.allocator();
        let mut frame_deallocator = |frame| allocator.deallocate_frame(frame);

        self.table().unmap(page, &mut frame_deallocator)
    }

    pub fn remap(&mut self, page: Page<Size4KB>, flags: PageTableFlags) -> Result<MapperFlush<Size4KB>, FlagUpdateError> {
        self.table().update_flags(page, flags)
    }

    // pub fn swap(&mut self, x: Page<Size4KB>, y: Page<Size4KB>) -> Result<DoubleMapperFlush<Size4KB>, SwapPageError> {
    //     self.table().swap(x, y)
    // }

    pub fn translate(&self, page: Page<Size4KB>) -> Option<PhysFrame> {
        self.table().translate(page)
    }
}
