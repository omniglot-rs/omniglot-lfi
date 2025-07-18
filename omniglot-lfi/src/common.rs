use omniglot::alloc_tracker::AllocTracker;

pub struct OGLFIAllocTracker;

unsafe impl AllocTracker for OGLFIAllocTracker {
    fn is_valid(&self, _: *const (), _: usize) -> bool {
        todo!()
    }
    fn is_valid_mut(&self, _: *mut (), _: usize) -> bool {
        todo!()
    }
}
