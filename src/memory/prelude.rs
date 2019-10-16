use x86_64::structures::paging as pg;
pub use x86_64::structures::paging::PageSize;

// Address types
pub use x86_64::{PhysAddr, VirtAddr};

// Page size
pub type PageSizeType = pg::Size2MiB;
pub const PAGE_SIZE_BYTES: u64 = 0x200_000;

pub type Page = pg::Page<PageSizeType>;
pub type PageRange = pg::page::PageRange<PageSizeType>;
pub type PhysFrame = pg::PhysFrame<PageSizeType>;
pub type PhysFrameRange = pg::frame::PhysFrameRange<PageSizeType>;
pub type PhysFrameRangeInclusive = pg::frame::PhysFrameRangeInclusive<PageSizeType>;
trait Mapper = pg::Mapper<PageSizeType>;
// pub trait FrameAllocator = pg::FrameAllocator<PageSizeType>;
pub use x86_64::structures::paging::FrameAllocator;

/// Numeric value of `PT_PADDR` for static assertions
pub const PT_PADDR_INT: u64 = 0x10_000_000;

/// Physical address of the page table area
/// This pointer itself points to P4 table.
pub const PT_PADDR: PhysAddr = unsafe { PhysAddr::new_unchecked(PT_PADDR_INT) };

// Require P2 alignment
static_assertions::const_assert!(PT_PADDR_INT % 0x1_000_000 == 0);

/// Numeric value of `PT_VADDR` for static assertions
pub const PT_VADDR_INT: u64 = 0x10_000_000;

/// Page tables are mapped starting from this virtual address.
/// This pointer itself points to P4 table.
pub const PT_VADDR: VirtAddr = unsafe { VirtAddr::new_unchecked_raw(PT_VADDR_INT) };

// Require P2 alignment
static_assertions::const_assert!(PT_VADDR_INT % 0x1_000_000 == 0);

/// Size of 2MiB huge page, in bytes
pub const HUGE_PAGE_SIZE: u64 = 0x200_000;
