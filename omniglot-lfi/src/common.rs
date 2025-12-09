// -*- fill-column: 80; -*-

use omniglot::alloc_tracker::AllocTracker;

pub struct AllowedRegions {
    // Because `free` only supplies a pointer and not a length, we only support
    // one region per unique start address (although regions are allowed to
    // overlap). We use a HashMap containing <start, (len, mutable)>, which
    // allows us to enforce this constraint and support efficient revoke
    // operations:
    regions: std::collections::HashMap<*mut (), (usize, bool)>,
}

impl AllowedRegions {
    pub fn new() -> Self {
        AllowedRegions {
            regions: std::collections::HashMap::new(),
        }
    }

    pub fn insert(&mut self, start: *mut (), len: usize, mutable: bool) -> bool {
        // If we already have a region for the same start address, return false,
        // and don't modify the set of regions.
        //
        // TODO: use `map_try_insert` once that is stabilized.
        if self.regions.contains_key(&start) {
            false
        } else {
            self.regions.insert(start, (len, mutable));
            true
        }
    }

    pub fn remove(&mut self, start: *mut ()) -> bool {
        self.regions.remove(&start).is_some()
    }

    pub fn is_valid_int(&self, start: *mut (), len: usize, mutable: bool) -> bool {
        if len == 0 {
            return true;
        }

        self.regions
            .iter()
            .find(|(region_start, (region_len, region_mutable))| {
                (!mutable || *region_mutable)
                    && **region_start as usize <= start as usize
                    && (**region_start as usize).saturating_add(*region_len)
                        >= (start as usize).saturating_add(len)
            })
            .is_some()
    }
}

pub enum AllocChain<'a> {
    Base(std::cell::RefCell<AllowedRegions>),
    Cons(&'a AllocChain<'a>),
    HostAllocation(&'a AllocChain<'a>, Allocation),
}

impl<'a> AllocChain<'a> {
    fn iter(&'a self) -> AllocChainIter<'a> {
        AllocChainIter(Some(self))
    }

    fn is_valid_int(&self, ptr: *mut (), len: usize, mutable: bool) -> bool {
        self.iter().any(|elem| match elem {
            AllocChain::Base(allowed_regions) => {
                allowed_regions.borrow().is_valid_int(ptr, len, mutable)
            }
            AllocChain::HostAllocation(_, alloc) => alloc.is_valid_int(ptr, len, mutable),
            AllocChain::Cons(_) => false,
        })
    }

    pub fn allowed_regions(&self) -> &std::cell::RefCell<AllowedRegions> {
        self.iter()
            .find_map(|elem| match elem {
                AllocChain::Base(allowed_regions) => Some(allowed_regions),
                _ => None,
            })
            .unwrap()
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
                AllocChain::Base(_) => None,
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
