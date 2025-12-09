// -*- fill-column: 80; -*-

// Prelude:
use omniglot::id::OGID;
use omniglot::markers::{AccessScope, AllocScope};

// Auto-generated bindings, so doesn't follow Rust conventions at all:
#[allow(non_upper_case_globals)]
#[allow(non_snake_case)]
#[allow(non_camel_case_types)]
#[allow(dead_code)]
#[allow(improper_ctypes)] // TODO: fix this by wrapping functions with u128s
pub mod libogadd {
    include!(concat!(env!("OUT_DIR"), "/libogadd_bindings.rs"));
}

// These are the Omniglot wrapper types / traits generated.
use libogadd::{LibOGAdd, LibOGAddRt};

pub fn with_lfi_sysv_amd64_rt_lib<ID: OGID, R>(
    brand: ID,
    f: impl for<'a> FnOnce(
        LibOGAddRt<ID, omniglot_lfi::amd64::OGLFISysVAMD64Runtime<ID>, omniglot_lfi::amd64::OGLFISysVAMD64Runtime<ID>>,
        AllocScope<
            <omniglot_lfi::amd64::OGLFISysVAMD64Runtime<ID> as omniglot::rt::OGRuntime>::AllocTracker<'a>,
            ID,
        >,
        AccessScope<ID>,
    ) -> R,
) -> R {
    let (rt, alloc, access) = omniglot_lfi::amd64::OGLFISysVAMD64Runtime::from_lfi_lib_bytes(
        include_bytes!("../libadd_lfi/libadd.lfi"),
        c"libadd".into(),
        [].into_iter(),
        // allow foreign library to use allow/revoke cbs to control which memory
        // the host can access:
        true,
        brand,
    )
    .unwrap();

    // Create a "bound" runtime, which implements the LibOGAdd API:
    let bound_rt = LibOGAddRt::new(rt)
        .expect("Failed to create bound runtime, likely problem with symbol resolution!");

    // Run the provided closure:
    f(bound_rt, alloc, access)
}

fn main() {
    env_logger::init();

    omniglot::id::lifetime::OGLifetimeBranding::new(|brand| {
        with_lfi_sysv_amd64_rt_lib(brand, |lib, mut alloc, mut access| {
            println!(
                "add(1, 2) = {}",
                lib.add(1, 2, &mut alloc, &mut access)
                    .expect("Error executing add function")
                    .validate()
                    .expect("Error validating returned value")
            );
        });
    });
}
