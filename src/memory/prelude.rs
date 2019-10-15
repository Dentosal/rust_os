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
