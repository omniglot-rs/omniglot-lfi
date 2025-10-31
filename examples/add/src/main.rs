// Necessary evil:
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

// pub fn with_mockrt_lib<'a, ID: OGID + 'a, A: omniglot::rt::mock::MockRtAllocator, R>(
//     brand: ID,
//     allocator: A,
//     f: impl FnOnce(
//         LibOGAddRt<ID, omniglot::rt::mock::MockRt<ID, A>, omniglot::rt::mock::MockRt<ID, A>>,
//         AllocScope<
//             <omniglot::rt::mock::MockRt<ID, A> as omniglot::rt::OGRuntime>::AllocTracker<'a>,
//             ID,
//         >,
//         AccessScope<ID>,
//     ) -> R,
// ) -> R {
//     // This is unsafe, as it instantiates a runtime that can be used to run
//     // foreign functions without memory protection:
//     let (rt, alloc, access) =
//         unsafe { omniglot::rt::mock::MockRt::new(false, false, allocator, brand) };

//     // Create a "bound" runtime, which implements the LibOGAdd API:no
//     let bound_rt = LibOGAddRt::new(rt).unwrap();

//     // Run the provided closure:
//     f(bound_rt, alloc, access)
// }

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
    let library_path = std::ffi::CString::new(concat!(env!("OUT_DIR"), "/libogadd.so")).unwrap();

    let (rt, alloc, access) = omniglot_lfi::amd64::OGLFISysVAMD64Runtime::from_lfi_lib_bytes(
        include_bytes!("../libadd_lfi/libadd.lfi"),
        c"libadd".into(),
        [].into_iter(),
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
