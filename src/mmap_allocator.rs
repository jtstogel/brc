use allocator_api2::alloc::{AllocError, Allocator, Layout};
use libc;
use std::ptr::NonNull;

/// Allocator that directly calls mmap/munmap for each allocation/deallocation.
pub struct MmapAllocator {
    request_hugepage: bool,
}

pub struct AllocatorOptions {
    pub request_hugepage: bool,
}

impl MmapAllocator {
    pub fn new(opts: &AllocatorOptions) -> Self {
        Self {
            request_hugepage: opts.request_hugepage,
        }
    }
}

unsafe impl Allocator for MmapAllocator {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        let size = layout.size().max(1);

        let ptr = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                size,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
                -1,
                0,
            )
        };

        if ptr == libc::MAP_FAILED {
            return Err(AllocError);
        }

        if self.request_hugepage {
            unsafe {
                libc::madvise(ptr, size, libc::MADV_HUGEPAGE);
            }
        }

        let nn = NonNull::new(ptr as *mut u8).ok_or(AllocError)?;
        Ok(NonNull::slice_from_raw_parts(nn, size))
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        let size = layout.size().max(1);
        unsafe { libc::munmap(ptr.as_ptr() as *mut libc::c_void, size) };
    }
}
