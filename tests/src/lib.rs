// Prelude:
use omniglot::id::OGID;
use omniglot::markers::{AccessScope, AllocScope};

// Auto-generated bindings, so doesn't follow Rust conventions at all:
#[allow(non_upper_case_globals)]
#[allow(non_snake_case)]
#[allow(non_camel_case_types)]
#[allow(dead_code)]
#[allow(improper_ctypes)] // TODO: fix this by wrapping functions with u128s
pub mod liboglfitests {
    include!(concat!(env!("OUT_DIR"), "/liboglfitests_bindings.rs"));
}

// Tests of `add{0..9}` functions, which check that arguments are
// properly copied and the invoke trampolines have access to the
// runtime struct and return value contexts:
mod add;

// Tests of the `stack_alloc` routines:
mod stack_alloc;

pub fn with_lfi_sysv_amd64_rt_lib<ID: OGID, R>(
    brand: ID,
    f: impl for<'a> FnOnce(
        liboglfitests::LibOGLFITestsRt<
            ID,
            omniglot_lfi::amd64::OGLFISysVAMD64Runtime<ID>,
            omniglot_lfi::amd64::OGLFISysVAMD64Runtime<ID>
        >,
        AllocScope<
            <
                omniglot_lfi::amd64::OGLFISysVAMD64Runtime<ID>
                    as omniglot::rt::OGRuntime
            >::AllocTracker<'a>,
            ID,
        >,
        AccessScope<ID>,
    ) -> R,
) -> R {
    let (rt, alloc, access) = omniglot_lfi::amd64::OGLFISysVAMD64Runtime::from_lfi_lib_bytes(
        include_bytes!("../liboglfitests_lfi/liboglfitests.lfi"),
        c"liboglfitests".into(),
        [].into_iter(),
        brand,
    )
    .unwrap();

    // Create a "bound" runtime, which implements the LibOGLFITests API:
    let bound_rt = liboglfitests::LibOGLFITestsRt::new(rt)
        .expect("Failed to create bound runtime, likely problem with symbol resolution!");

    // Run the provided closure:
    f(bound_rt, alloc, access)
}
