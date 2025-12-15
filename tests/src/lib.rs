// -*- fill-column: 80; -*-

// Prelude:
use omniglot::id::OGID;
use omniglot::markers::{AccessScope, AllocScope};

use omniglot_lfi::OGLFIMemoryAccessConfig;

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

// Callback-related tests:
mod callbacks;

// Tests of the `stack_alloc` routines:
mod stack_alloc;

// Tests of heap memory allocation and allow/revoke boxrt callbacks:
mod malloc;

// Tests exercising stack overflow handling:
mod stack_overflow;

// Helper function to initialize the env_logger crate, called from the
// other `with_*_rt_lib` helper(s) in this file:
pub fn env_logger_init() {
    let _ = env_logger::builder().is_test(true).try_init();
}

pub trait OGLFITestConfiguration {
    const BINARY: &'static [u8];
    const MEMORY_ACCESS_CFG: OGLFIMemoryAccessConfig;
}

pub enum OGLFITestDefaultConfig {}
impl OGLFITestConfiguration for OGLFITestDefaultConfig {
    const BINARY: &'static [u8] = include_bytes!("../liboglfitests_lfi/liboglfitests_default.lfi");
    const MEMORY_ACCESS_CFG: OGLFIMemoryAccessConfig =
        OGLFIMemoryAccessConfig::ALL_MEMORY_ACCESSIBLE;
}

pub enum OGLFITestAutoAllowRevokeConfig {}
impl OGLFITestConfiguration for OGLFITestAutoAllowRevokeConfig {
    const BINARY: &'static [u8] =
        include_bytes!("../liboglfitests_lfi/liboglfitests_auto_allow_revoke.lfi");
    const MEMORY_ACCESS_CFG: OGLFIMemoryAccessConfig =
        OGLFIMemoryAccessConfig::STACK_OR_REQUIRE_ALLOW_REVOKE;
}

// Helper function, to load the tests library into LFI and create an
// Omniglot-LFI wrapper instance:
pub fn with_lfi_sysv_amd64_rt_lib<CFG: OGLFITestConfiguration, ID: OGID, R>(
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
    env_logger_init();

    let (rt, alloc, access) = omniglot_lfi::amd64::OGLFISysVAMD64Runtime::from_lfi_lib_bytes(
        CFG::BINARY,
        c"liboglfitests".into(),
        [].into_iter(),
        CFG::MEMORY_ACCESS_CFG,
        brand,
    )
    .unwrap();

    // Create a "bound" runtime, which implements the LibOGLFITests API:
    let bound_rt = liboglfitests::LibOGLFITestsRt::new(rt)
        .expect("Failed to create bound runtime, likely problem with symbol resolution!");

    // Run the provided closure:
    f(bound_rt, alloc, access)
}
