// -*- fill-column: 80; -*-

/// Tests exercising stack overflow cases during invoke.
///
/// When invoking a foreign function, the invoke trampoline copies STACK_SPILL
/// bytes from the host stack to the sandbox stack. To do so, it first moves the
/// sandbox stack down by STACK_SPILL (+ alignment) bytes, and then performs a
/// copy from the host stack. `STACK_SPILL` is considered trusted, and so should
/// never be larger than the actual host stack.
///
/// There are two cases where an overflow (underflow) can occur:
///
/// - When computing the new sandbox stack as `new = old - STACK_SPILL`, this
///   subtraction can underflow. We exercise this case through the
///   `test_stack_sub_underflow` test case.
///
/// - When computing the new sandbox stack as `new = old - STACK_SPILL` the
///   subtraction does *not* underflow, but `new < LFI box min addr`. In that
///   case, the invoke trampoline would copy over memory outside the sandbox,
///   which we must avoid.
///
///   To exercise this case, we need to select `STACK_SPILL` as smaller than
///   `old`, but large enough to move `new` out of the sandbox
///   boundaries. However, because `STACK_SPILL` is a constant, we cannot do
///   this at runtime.
use std::ffi::CStr;
use std::ffi::c_void;

use omniglot::OGError;
use omniglot::abi::calling_convention::AREG0;
use omniglot::abi::sysv_amd64::SysVAMD64ABI;
use omniglot::id::OGID;
use omniglot::markers::{AccessScope, AllocScope};
use omniglot::rt::OGRuntime;
use omniglot::rt::sysv_amd64::SysVAMD64BaseRt;
use omniglot::rt::sysv_amd64::SysVAMD64InvokeRes;
use omniglot::rt::sysv_amd64::SysVAMD64Rt;

use omniglot_lfi::amd64::{OGLFISysVAMD64InvokeRes, OGLFISysVAMD64Runtime};

use crate::liboglfitests::{LibOGLFITests, LibOGLFITestsRt};

#[allow(dead_code)]
fn invoke_add0_stack_spill_override<const STACK_SPILL: usize, ID: OGID>(
    lib: &LibOGLFITestsRt<ID, OGLFISysVAMD64Runtime<ID>, OGLFISysVAMD64Runtime<ID>>,
    alloc_scope: &mut AllocScope<<OGLFISysVAMD64Runtime<ID> as OGRuntime>::AllocTracker<'_>, ID>,
    access_scope: &mut AccessScope<ID>,
) -> Option<OGError> {
    const ADD0_SYM_NAME: &'static CStr = c"add0";

    let add0_sym_addr = lib
        .rt()
        .resolve_symbols(&[ADD0_SYM_NAME], &[])
        .ok()
        .and_then(|sym_tab| {
            lib.rt().lookup_symbol::<_, 0>(
                0, // compact symbol table index,
                0, // fixed offset symbol table index, unused,
                &sym_tab,
            )
        })
        .expect("Failed to look up add0 symbol address!");

    #[unsafe(naked)]
    unsafe extern "C" fn add0_trampoline<
        const STACK_SPILL: usize,
        // No stack spill, runtime passed in third argument register:
        RT: SysVAMD64Rt<STACK_SPILL, AREG0<SysVAMD64ABI>>,
    >(
        // Actual C function arguments (none):
        // Additional Omniglot trampoline arguments:
        _rt: &RT,
        _fnptr_unused: *const (), // not used for LFI
        _resptr: &mut <RT as SysVAMD64BaseRt>::InvokeRes<c_void>,
    ) {
        core::arch::naked_asm!(
            "jmp {invoke}",
            invoke = sym RT::invoke,
        );
    }

    let mut res = OGLFISysVAMD64InvokeRes::<OGLFISysVAMD64Runtime<ID>, c_void>::new();
    lib.rt()
        .execute(add0_sym_addr, alloc_scope, access_scope, || unsafe {
            add0_trampoline::<STACK_SPILL, _>(lib.rt(), core::ptr::null(), &mut res);
        })
        .expect("Execute failed!");

    res.into_result_registers(lib.rt()).err()
}

#[test]
fn test_no_stack_overflow() {
    omniglot::id::lifetime::OGLifetimeBranding::new(|brand| {
        crate::with_lfi_sysv_amd64_rt_lib::<crate::OGLFITestDefaultConfig, _, _>(
            brand,
            |lib, mut alloc, mut access| {
                let err = invoke_add0_stack_spill_override::<
                    // No stack spill:
                    0,
                    _,
                >(&lib, &mut alloc, &mut access);

                assert!(err.is_none());
            },
        )
    });
}

#[test]
fn test_stack_sub_underflow() {
    omniglot::id::lifetime::OGLifetimeBranding::new(|brand| {
        crate::with_lfi_sysv_amd64_rt_lib::<crate::OGLFITestDefaultConfig, _, _>(
            brand,
            |lib, mut alloc, mut access| {
                let err = invoke_add0_stack_spill_override::<
                    // Guaranteed subtraction underflow while trying to compute
                    // how much stacked data to copy:
                    { usize::MAX },
                    _,
                >(&lib, &mut alloc, &mut access);

                assert_eq!(err, Some(OGError::StackOverflow));
            },
        )
    });
}

#[test]
fn test_stack_sandbox_underflow() {
    omniglot::id::lifetime::OGLifetimeBranding::new(|brand| {
        crate::with_lfi_sysv_amd64_rt_lib::<crate::OGLFITestDefaultConfig, _, _>(
            brand,
            |lib, mut alloc, mut access| {
                // TODO: this is not reliable right now. We assume that the
                // sandbox is loaded at an address higher than `STACK_SPILL`
                // (such that the subtraction doesn't overflow), but that
                // `STACK_SPILL` is larger than the sandbox's memory.
                const STACK_SPILL: usize = 0x0000100000000000;

                // Retrieve the sandbox stack pointer:
                let current_sandbox_sp: *mut c_void = lib
                    .current_sandbox_sp(&mut alloc, &mut access)
                    .expect("Failed to run sandbox function to determine current stack pointer")
                    .valid_ptr();
                println!("Current sandbox stack pointer: {:p}", current_sandbox_sp);

                // Ensure that we're not underflowing the new sandbox stack
                // pointer calculation:
                let new_sp = (current_sandbox_sp as usize)
                    .checked_sub(STACK_SPILL)
                    .expect("Underflow in stack pointer calculation");
                println!(
                    "Subtracting stack spill of {} bytes, new stack pointer: {:p}",
                    STACK_SPILL, new_sp as *mut c_void
                );

                // If this SIGSEVs, probably need to increase `STACK_SPILL`. In
                // the future, we should be querying the `box_min_addr` from
                // within the runtime and use that to choose a set of
                // pre-computed `STACK_SPILL` trampolines that is guaranteed to
                // overflow the sandbox boundary:
                let err = invoke_add0_stack_spill_override::<{ STACK_SPILL }, _>(
                    &lib,
                    &mut alloc,
                    &mut access,
                );

                assert_eq!(err, Some(OGError::StackOverflow));
            },
        )
    });
}

// #[test]
// fn test_malloc_free_no_allow_revoke_callbacks() {
//     omniglot::id::lifetime::OGLifetimeBranding::new(|brand| {
//         crate::with_lfi_sysv_amd64_rt_lib::<crate::OGLFITestDefaultConfig, _, _>(
//             brand,
//             |lib, mut alloc, mut access| {
//                 use crate::liboglfitests::LibOGLFITests;
//                 // use omniglot::foreign_memory::og_mut_slice::OGMutSlice;

//                 println!("Allocating memory on the library heap with malloc:");
//                 const LEN: usize = 42;
//                 let _ptr = lib.rt().malloc(LEN, &mut alloc, &mut access).unwrap();

//                 // TODO: this is currently broken. When not using the
//                 // allow/revoke callbacks, upgrade by default only has access to
//                 // host-stacked memory.

//                 // println!("Got {} bytes at {:p}, trying to upgrade:", LEN, ptr);
//                 // let slice = OGMutSlice::upgrade_from_ptr(ptr as *mut u8, LEN, &mut alloc)
//                 //     .expect("Failed to upgrade memory allocated with `malloc`!");

//                 // println!("Got OGMutSlice over memory, filling with `42`s...");
//                 // slice.write_from_iter(std::iter::repeat(42), &mut access);

//                 // println!("Freeing memory backing the OGMutSlice...");
//                 // lib.rt().free(ptr, &mut alloc, &mut access).unwrap();

//                 // // No longer able to access `slice` here, unless the
//                 // // `alloc_scope_separate_active_valid_lt` feature on the `omniglot`
//                 // // crate is enabled, which will in turn prevent using the
//                 // // `allow_boxrt_protection` option for `omniglot-lfi`.
//                 // //
//                 // // println!("Writing to slice again after freed.");
//                 // // slice.write_from_iter(std::iter::repeat(42), &mut access);

//                 // println!("Trying to upgrade a second time...");
//                 // assert!(OGMutSlice::upgrade_from_ptr(ptr as *mut u8, LEN, &mut alloc).is_some());
//                 // println!("As expected, upgrade still worked -- allow and revoke callback are disabled")
//             },
//         )
//     });
// }
