// -*- fill-column: 80; -*-

use std::ptr::null;

use omniglot::id::OGID;
use omniglot::markers::{AccessScope, AllocScope};
use omniglot::rt::OGRuntime;

// Auto-generated bindings, so doesn't follow Rust conventions at all:
#[allow(non_upper_case_globals)]
#[allow(non_snake_case)]
#[allow(non_camel_case_types)]
#[allow(dead_code)]
#[allow(improper_ctypes)] // TODO: fix this by wrapping functions with u128s
pub mod sodium_bindings {
    include!(concat!(env!("OUT_DIR"), "/sodium_bindings.rs"));
}

use sodium_bindings::{crypto_generichash, Sodium, SodiumRt};

pub fn sodium_hash_unsafe(message: &[u8]) -> [u8; 32] {
    let hash = [0 as u8; 32];
    let res = unsafe {
        crypto_generichash(
            hash.as_ptr() as *mut u8,
            32,
            message.as_ptr() as *const u8,
            message.len() as u64,
            null(),
            0,
        )
    };

    assert!(res == 0);

    hash
}

pub fn sodium_hash_og<ID: OGID, RT: OGRuntime<ID = ID>, L: Sodium<ID, RT, RT = RT>>(
    lib: &L,
    alloc: &mut AllocScope<RT::AllocTracker<'_>, RT::ID>,
    access: &mut AccessScope<RT::ID>,
    message: &[u8],
    result_cb: impl FnOnce(&[u8]),
) {
    lib.rt()
        .allocate_stacked_slice_mut::<u8, _, _>(message.len(), alloc, |message_ref, alloc| {
            // Initialize the EFAllocation into an EFMutVal:
            message_ref.copy_from_slice(message, access);

            lib.rt()
                .allocate_stacked_t_mut::<[u8; 32], _, _>(alloc, |hash_ref, alloc| {
                    let res = lib
                        .crypto_generichash(
                            hash_ref.as_ptr().cast::<u8>(),
                            32,
                            message_ref.as_ptr(),
                            message.len() as u64,
                            null(),
                            0,
                            alloc,
                            access,
                        )
                        .unwrap()
                        .valid();

                    assert!(res == 0);

                    result_cb(&*hash_ref.validate(&access).unwrap())
                })
                .unwrap();
        })
        .unwrap();
}

pub fn with_mockrt_lib<'a, ID: OGID + 'a, A: omniglot::rt::mock::MockRtAllocator, R>(
    brand: ID,
    allocator: A,
    f: impl FnOnce(
        SodiumRt<ID, omniglot::rt::mock::MockRt<ID, A>, omniglot::rt::mock::MockRt<ID, A>>,
        AllocScope<
            <omniglot::rt::mock::MockRt<ID, A> as omniglot::rt::OGRuntime>::AllocTracker<'a>,
            ID,
        >,
        AccessScope<ID>,
    ) -> R,
) -> R {
    // This is unsafe, as it instantiates a runtime that can be used to run
    // foreign functions without memory protection:
    let (rt, mut alloc, mut access) =
        unsafe { omniglot::rt::mock::MockRt::new(false, false, allocator, brand) };

    // Create a "bound" runtime, which implements the Sodium API:
    let bound_rt = SodiumRt::new(rt).unwrap();

    // All further functions expect sodium to be initialized:
    println!("Initializing sodium for MockRt...");
    assert!(
        0 == bound_rt
            .sodium_init(&mut alloc, &mut access)
            .unwrap()
            .valid()
    );
    println!("MockRt sodium initialized!");

    // Run the provided closure:
    f(bound_rt, alloc, access)
}

pub fn with_lfi_sysv_amd64_rt_lib<ID: OGID, R>(
    brand: ID,
    f: impl for<'a> FnOnce(
        SodiumRt<ID, omniglot_lfi::amd64::OGLFISysVAMD64Runtime<ID>, omniglot_lfi::amd64::OGLFISysVAMD64Runtime<ID>>,
        AllocScope<
            <omniglot_lfi::amd64::OGLFISysVAMD64Runtime<ID> as omniglot::rt::OGRuntime>::AllocTracker<'a>,
            ID,
        >,
        AccessScope<ID>,
    ) -> R,
) -> R {
    let (rt, mut alloc, mut access) =
        omniglot_lfi::amd64::OGLFISysVAMD64Runtime::from_lfi_lib_bytes(
            include_bytes!(concat!(
                env!("OG_SODIUM_LFI_BUILD_PATH"),
                "/og_sodium_mimalloc_default.lfi"
            )),
            c"sodium".into(),
            [].into_iter(),
            omniglot_lfi::OGLFIMemoryAccessConfig::ALL_MEMORY_ACCESSIBLE,
            brand,
        )
        .unwrap();

    // Create a "bound" runtime, which implements the Sodium API:
    let bound_rt = SodiumRt::new(rt)
        .expect("Failed to create bound runtime, likely problem with symbol resolution!");

    // All further functions expect sodium to be initialized:
    println!("Initializing sodium in LFI sandbox...");
    assert!(
        0 == bound_rt
            .sodium_init(&mut alloc, &mut access)
            .unwrap()
            .valid()
    );
    println!("LFI sandbox sodium initialized!");

    // Run the provided closure:
    f(bound_rt, alloc, access)
}
