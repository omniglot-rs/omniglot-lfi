// -*- fill-column: 80; -*-

use std::cell::RefCell;
use std::ffi::c_char;

use omniglot::id::runtime::OGRuntimeBranding;
use omniglot::markers::{AccessScope, AllocScope};
use omniglot::rt::OGRuntime;
use omniglot_lfi::amd64::OGLFISysVAMD64Runtime;

// Auto-generated bindings, so doesn't follow Rust conventions at all:
#[allow(non_upper_case_globals)]
#[allow(non_snake_case)]
#[allow(non_camel_case_types)]
#[allow(dead_code)]
pub mod llhttp_bindings {
    include!(concat!(env!("OUT_DIR"), "/llhttp_bindings.rs"));
}

use crate::llhttp_bindings::{Llhttp, LlhttpRt};

type LlhttpOGRuntimeScopes = (
    LlhttpRt<
        OGRuntimeBranding,
        OGLFISysVAMD64Runtime<OGRuntimeBranding>,
        OGLFISysVAMD64Runtime<OGRuntimeBranding>,
    >,
    AllocScope<
        'static,
        <OGLFISysVAMD64Runtime<OGRuntimeBranding> as OGRuntime>::AllocTracker<'static>,
        OGRuntimeBranding,
    >,
    AccessScope<OGRuntimeBranding>,
);

pub fn new_lfi_sysv_amd64_rt_lib() -> (LlhttpOGRuntimeScopes, *mut llhttp_bindings::llhttp_t) {
    let brand = OGRuntimeBranding::new();

    let (rt, mut alloc, mut access) = OGLFISysVAMD64Runtime::from_lfi_lib_bytes(
        include_bytes!(concat!(
            env!("OG_LLHTTP_LFI_BUILD_PATH"),
            "/og_llhttp_mimalloc_default.lfi"
        )),
        c"llhttp".into(),
        [].into_iter(),
        // Don't expose allow/revoke callbacks to the foreign library:
        false,
        brand,
    )
    .unwrap();

    // Create a "bound" runtime, which implements the Llhttp API:
    let lib = LlhttpRt::new(rt)
        .expect("Failed to create bound runtime, likely problem with symbol resolution!");

    // We `malloc` the `llhttp_t` parser and the `llhttp_settings_t` struct, so
    // that we don't have to re-initialize every time we enter into this
    // function:
    let parser = lib
        .rt()
        .malloc(
            std::mem::size_of::<llhttp_bindings::llhttp_t>(),
            &mut alloc,
            &mut access,
        )
        .unwrap()
        .cast::<llhttp_bindings::llhttp_t>();

    let settings = lib
        .rt()
        .malloc(
            std::mem::size_of::<llhttp_bindings::llhttp_settings_t>(),
            &mut alloc,
            &mut access,
        )
        .unwrap()
        .cast::<llhttp_bindings::llhttp_settings_t>();

    // Initialize the settings struct:
    lib.llhttp_settings_init(settings, &mut alloc, &mut access)
        .unwrap();

    // Initialize the parser in HTTP_BOTH mode, meaning that it will select
    // between HTTP_REQUEST and HTTP_RESPONSE parsing automatically while
    // reading the first input.
    lib.llhttp_init(
        parser,
        llhttp_bindings::llhttp_type_HTTP_BOTH,
        settings,
        &mut alloc,
        &mut access,
    )
    .unwrap();

    // Return the runtime and scopes:
    ((lib, alloc, access), parser)
}

thread_local! {
    static LLHTTP_OG_RT: RefCell<(LlhttpOGRuntimeScopes, *mut llhttp_bindings::llhttp_t)> =
        RefCell::new(new_lfi_sysv_amd64_rt_lib());
}

pub fn parse_http_request(req: &[u8]) {
    LLHTTP_OG_RT.with_borrow_mut(|((lib, alloc, access), parser)| {
        // Parse request, which we copy onto the stack:
        lib.rt()
            .write_stacked_slice(req, alloc, access, |req_slice, alloc, access| {
                assert_eq!(
                    0,
                    lib.llhttp_execute(
                        *parser,
                        req_slice.as_ptr().cast::<c_char>(),
                        req_slice.len(),
                        alloc,
                        access
                    )
                    .unwrap()
                    .valid()
                );
            })
            .unwrap();
    });
}

#[test]
fn smoketest() {
    parse_http_request(include_bytes!("../get_wikipedia_org_req.txt"))
}
