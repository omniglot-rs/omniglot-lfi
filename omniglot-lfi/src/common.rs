// -*- fill-column: 80; -*-

use omniglot::alloc_tracker::AllocTracker;

use crate::OGLFIMemoryAccessConfig;

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

pub struct AllocChainBase {
    allowed: std::cell::RefCell<AllowedRegions>,
    memory_access_cfg: OGLFIMemoryAccessConfig,
    box_min_addr: *mut (),
    box_max_addr: *mut (),
    foreign_stack_top: *mut (),
}

pub enum AllocChain<'a> {
    Base(AllocChainBase),
    Cons {
        base: &'a AllocChainBase,
        pred: &'a AllocChain<'a>,
    },
    ForeignStack {
        base: &'a AllocChainBase,
        pred: &'a AllocChain<'a>,
        foreign_stack_bottom: *mut (),
    },
    HostAllocation {
        base: &'a AllocChainBase,
        pred: &'a AllocChain<'a>,
        alloc: Allocation,
    },
}

impl<'a> AllocChain<'a> {
    pub fn new(
        memory_access_cfg: OGLFIMemoryAccessConfig,
        box_min_addr: *mut (),
        box_max_addr: *mut (),
        foreign_stack_top: *mut (),
    ) -> AllocChain<'a> {
        AllocChain::Base(AllocChainBase {
            allowed: std::cell::RefCell::new(AllowedRegions::new()),
            memory_access_cfg,
            box_min_addr,
            box_max_addr,
            foreign_stack_top,
        })
    }

    fn iter(&'a self) -> AllocChainIter<'a> {
        AllocChainIter(Some(self))
    }

    fn is_valid_int(&self, ptr: *mut (), len: usize, mutable: bool) -> bool {
        let base = self.base();

        // If we have access to all sandbox memory, just check that the access
        // is contained in the sandbox:
        if base.memory_access_cfg.enable_all_sandbox_memory_access
            && (ptr as usize) >= (base.box_min_addr as usize)
            && (ptr as usize)
                .checked_add(len)
                .is_some_and(|end_ptr| end_ptr <= base.box_max_addr as usize)
        {
            return true;
        }

        self.iter().any(|elem| match elem {
            AllocChain::Base(base) if base.memory_access_cfg.enable_allowed_memory_access => {
                base.allowed.borrow().is_valid_int(ptr, len, mutable)
            }
            AllocChain::Base(_) => false,

            AllocChain::HostAllocation { alloc, .. } => alloc.is_valid_int(ptr, len, mutable),

            AllocChain::ForeignStack {
                foreign_stack_bottom,
                ..
            } if base.memory_access_cfg.enable_sandbox_stack_access => {
                // TODO: this should probably only check the very first
                // ForeignStack encountered with the iterator:
                ptr as usize <= self.base().foreign_stack_top as usize
                    && (ptr as usize)
                        .checked_add(len)
                        .is_some_and(|end_ptr| end_ptr > *foreign_stack_bottom as usize)
            }
            AllocChain::ForeignStack { .. } => false,

            AllocChain::Cons { .. } => false,
        })
    }

    pub fn base(&'a self) -> &'a AllocChainBase {
        match self {
            AllocChain::Base(base) => base,
            AllocChain::HostAllocation { base, .. } => base,
            AllocChain::ForeignStack { base, .. } => base,
            AllocChain::Cons { base, .. } => base,
        }
    }

    pub fn allowed_regions(&self) -> &std::cell::RefCell<AllowedRegions> {
        &self.base().allowed
    }
}

unsafe impl<'a> AllocTracker for AllocChain<'a> {
    fn is_valid(&self, ptr: *const (), len: usize) -> bool {
        let v = self.is_valid_int(ptr as *mut (), len, false);
        // println!("is valid? {:p}, {} = {:?}", ptr, len, v);
        v
    }

    fn is_valid_mut(&self, ptr: *mut (), len: usize) -> bool {
        let v = self.is_valid_int(ptr, len, true);
        // println!("is valid mut? {:p}, {} = {:?}", ptr, len, v);
        v
    }
}

struct AllocChainIter<'a>(Option<&'a AllocChain<'a>>);
impl<'a> Iterator for AllocChainIter<'a> {
    type Item = &'a AllocChain<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(cur) = self.0 {
            self.0 = match cur {
                AllocChain::Base { .. } => None,
                AllocChain::HostAllocation { pred, .. } => Some(pred),
                AllocChain::ForeignStack { pred, .. } => Some(pred),
                AllocChain::Cons { pred, .. } => Some(pred),
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
