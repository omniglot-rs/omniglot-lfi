// Necessary evil:
use omniglot::id::OGID;
use omniglot::markers::{AccessScope, AllocScope};

// Auto-generated bindings, so doesn't follow Rust conventions at all:
#[allow(non_upper_case_globals)]
#[allow(non_snake_case)]
#[allow(non_camel_case_types)]
#[allow(dead_code)]
#[allow(improper_ctypes)] // TODO: fix this by wrapping functions with u128s
pub mod libogdemo {
    include!(concat!(env!("OUT_DIR"), "/libogdemo_bindings.rs"));
}

// These are the Omniglot wrapper types / traits generated.
use libogdemo::LibOGDemoRt;

pub fn with_mockrt_lib<'a, ID: OGID + 'a, A: omniglot::rt::mock::MockRtAllocator, R>(
    brand: ID,
    allocator: A,
    f: impl FnOnce(
        LibOGDemoRt<ID, omniglot::rt::mock::MockRt<ID, A>, omniglot::rt::mock::MockRt<ID, A>>,
        AllocScope<
            <omniglot::rt::mock::MockRt<ID, A> as omniglot::rt::OGRuntime>::AllocTracker<'a>,
            ID,
        >,
        AccessScope<ID>,
    ) -> R,
) -> R {
    // This is unsafe, as it instantiates a runtime that can be used to run
    // foreign functions without memory protection:
    let (rt, alloc, access) =
        unsafe { omniglot::rt::mock::MockRt::new(false, false, allocator, brand) };

    // Create a "bound" runtime, which implements the LibOGDemo API:no
    let bound_rt = LibOGDemoRt::new(rt).unwrap();

    // Run the provided closure:
    f(bound_rt, alloc, access)
}

pub fn with_lfi_sysv_amd64_rt_lib<ID: OGID, R>(
    brand: ID,
    f: impl for<'a> FnOnce(
        LibOGDemoRt<ID, omniglot_lfi::amd64::OGLFISysVAMD64Runtime<ID>, omniglot_lfi::amd64::OGLFISysVAMD64Runtime<ID>>,
        AllocScope<
            <omniglot_lfi::amd64::OGLFISysVAMD64Runtime<ID> as omniglot::rt::OGRuntime>::AllocTracker<'a>,
            ID,
        >,
        AccessScope<ID>,
    ) -> R,
) -> R {
    let (rt, alloc, access) = omniglot_lfi::amd64::OGLFISysVAMD64Runtime::from_lfi_lib_bytes(
        include_bytes!("../liboglfiubench_lfi/liboglfiubench_mimalloc_default.lfi"),
        c"omniglot-lfi-microbenchmarks".into(),
        [].into_iter(),
        omniglot_lfi::OGLFIMemoryAccessConfig::ALL_MEMORY_ACCESSIBLE,
        brand,
    )
    .unwrap();

    // Create a "bound" runtime, which implements the Brotli API:
    let bound_rt = LibOGDemoRt::new(rt)
        .expect("Failed to create bound runtime, likely problem with symbol resolution!");

    // Run the provided closure:
    f(bound_rt, alloc, access)
}
