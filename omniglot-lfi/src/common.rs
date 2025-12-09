// -*- fill-column: 80; -*-

use omniglot::alloc_tracker::AllocTracker;

pub enum AllocChain<'a> {
    Base,
    Cons(&'a AllocChain<'a>),
    HostAllocation(&'a AllocChain<'a>, Allocation),
}

impl<'a> AllocChain<'a> {
    fn iter(&'a self) -> AllocChainIter<'a> {
        AllocChainIter(Some(self))
    }

    fn is_valid_int(&self, ptr: *mut (), len: usize, mutable: bool) -> bool {
        self.iter().any(|elem| match elem {
            AllocChain::Base => {
                // TODO:
                false
            }
            AllocChain::HostAllocation(_, alloc) => alloc.is_valid_int(ptr, len, mutable),
            AllocChain::Cons(_) => false,
        })
    }
}

unsafe impl<'a> AllocTracker for AllocChain<'a> {
    fn is_valid(&self, ptr: *const (), len: usize) -> bool {
        self.is_valid_int(ptr as *mut (), len, false)
    }

    fn is_valid_mut(&self, ptr: *mut (), len: usize) -> bool {
        self.is_valid_int(ptr, len, true)
    }
}

struct AllocChainIter<'a>(Option<&'a AllocChain<'a>>);
impl<'a> Iterator for AllocChainIter<'a> {
    type Item = &'a AllocChain<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(cur) = self.0 {
            self.0 = match cur {
                AllocChain::Base => None,
                AllocChain::HostAllocation(pred, _) => Some(pred),
                AllocChain::Cons(pred) => Some(pred),
            };

            Some(cur)
        } else {
            None
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Allocation {
    pub ptr: *mut (),
    pub len: usize,
    pub mutable: bool,
}

impl Allocation {
    fn is_valid_int(&self, ptr: *mut (), len: usize, mutable: bool) -> bool {
        (!mutable || self.mutable)
            && (ptr as usize) >= (self.ptr as usize)
            && ((ptr as usize)
                .checked_add(len)
                .map(|end| end <= (self.ptr as usize) + self.len)
                .unwrap_or(false))
    }
}
