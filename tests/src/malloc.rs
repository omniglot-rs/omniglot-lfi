// -*- fill-column: 80; -*-

#[test]
fn test_malloc_free_allow_revoke_callbacks() {
    omniglot::id::lifetime::OGLifetimeBranding::new(|brand| {
        crate::with_lfi_sysv_amd64_rt_lib::<crate::OGLFITestAutoAllowRevokeConfig, _, _>(
            brand,
            |lib, mut alloc, mut access| {
                use crate::liboglfitests::LibOGLFITests;
                use omniglot::foreign_memory::og_mut_slice::OGMutSlice;

                println!("Allocating memory on the library heap with malloc:");
                const LEN: usize = 42;
                let ptr = lib.rt().malloc(LEN, &mut alloc, &mut access).unwrap();

                println!("Got {} bytes at {:p}, trying to upgrade:", LEN, ptr);
                let slice = OGMutSlice::upgrade_from_ptr(ptr as *mut u8, LEN, &mut alloc)
                    .expect("Failed to upgrade memory allocated with `malloc`!");

                println!("Got OGMutSlice over memory, filling with `42`s...");
                slice.write_from_iter(std::iter::repeat(42), &mut access);

                println!("Freeing memory backing the OGMutSlice...");
                lib.rt().free(ptr, &mut alloc, &mut access).unwrap();

                // No longer able to access `slice` here, unless the
                // `alloc_scope_separate_active_valid_lt` feature on the `omniglot`
                // crate is enabled, which will in turn prevent using the
                // `allow_boxrt_protection` option for `omniglot-lfi`.
                //
                // println!("Writing to slice again after freed.");
                // slice.write_from_iter(std::iter::repeat(42), &mut access);

                println!("Trying to upgrade a second time...");
                assert!(OGMutSlice::upgrade_from_ptr(ptr as *mut u8, LEN, &mut alloc).is_none());
                println!("As expected, upgrade returned None")
            },
        )
    });
}

#[test]
fn test_malloc_free_no_allow_revoke_callbacks() {
    omniglot::id::lifetime::OGLifetimeBranding::new(|brand| {
        crate::with_lfi_sysv_amd64_rt_lib::<crate::OGLFITestDefaultConfig, _, _>(
            brand,
            |lib, mut alloc, mut access| {
                use crate::liboglfitests::LibOGLFITests;
                // use omniglot::foreign_memory::og_mut_slice::OGMutSlice;

                println!("Allocating memory on the library heap with malloc:");
                const LEN: usize = 42;
                let _ptr = lib.rt().malloc(LEN, &mut alloc, &mut access).unwrap();

                // TODO: this is currently broken. When not using the
                // allow/revoke callbacks, upgrade by default only has access to
                // host-stacked memory.

                // println!("Got {} bytes at {:p}, trying to upgrade:", LEN, ptr);
                // let slice = OGMutSlice::upgrade_from_ptr(ptr as *mut u8, LEN, &mut alloc)
                //     .expect("Failed to upgrade memory allocated with `malloc`!");

                // println!("Got OGMutSlice over memory, filling with `42`s...");
                // slice.write_from_iter(std::iter::repeat(42), &mut access);

                // println!("Freeing memory backing the OGMutSlice...");
                // lib.rt().free(ptr, &mut alloc, &mut access).unwrap();

                // // No longer able to access `slice` here, unless the
                // // `alloc_scope_separate_active_valid_lt` feature on the `omniglot`
                // // crate is enabled, which will in turn prevent using the
                // // `allow_boxrt_protection` option for `omniglot-lfi`.
                // //
                // // println!("Writing to slice again after freed.");
                // // slice.write_from_iter(std::iter::repeat(42), &mut access);

                // println!("Trying to upgrade a second time...");
                // assert!(OGMutSlice::upgrade_from_ptr(ptr as *mut u8, LEN, &mut alloc).is_some());
                // println!("As expected, upgrade still worked -- allow and revoke callback are disabled")
            },
        )
    });
}
