// -*- fill-column: 80; -*-

#[test]
fn test_stack_alloc_basic() {
    omniglot::id::lifetime::OGLifetimeBranding::new(|brand| {
        crate::with_lfi_sysv_amd64_rt_lib::<crate::OGLFITestDefaultConfig, _, _>(
            brand,
            |lib, mut alloc, mut access| {
                use crate::liboglfitests::LibOGLFITests;
                use omniglot::foreign_memory::og_mut_ref::OGMutRef;
                use omniglot::rt::OGRuntime;

                // Allocate some data on the stack, write a known-good value to
                // it. Then call a function which spills arguments on the
                // stack. After this function has run, re-create an OGMutRef from
                // the allocation pointer (to ensure it can be upgraded) and ensure
                // that the stacked values are still good.
                lib.rt()
                    .allocate_stacked_t_mut::<[u8; 12], _, _>(&mut alloc, |arr, alloc| {
                        const ARRAY_CONTENTS: &[u8] = b"Hello World!";

                        // Initialize with some known-good data:
                        arr.as_slice().copy_from_slice(ARRAY_CONTENTS, &mut access);

                        // Now, invoke a function that spills arguments onto the stack:
                        assert_eq!(
                            lib.add9(1, 2, 3, 4, 5, 6, 7, 8, 9, alloc, &mut access)
                                .expect("Error executing add9")
                                .validate()
                                .expect("Error validating add9 result"),
                            45,
                            "add9 returned incorrect result",
                        );

                        // Upgrade arr again from a raw pointer:
                        let arr_ptr: *mut [u8; 12] = arr.as_ptr();
                        let arr: OGMutRef<'_, _, [u8; 12]> = OGMutRef::upgrade_from_ptr(
                            arr_ptr, alloc,
                        )
                        .expect("Failed to upgrade pointer derived from stacked array allocation");

                        // Make sure that its contents have not been
                        // touched. Explicit assert over both slices' length to
                        // ensure that `.zip()` doesn't truncate:
                        assert_eq!(arr.as_slice().len(), ARRAY_CONTENTS.len());
                        arr.as_slice().iter().zip(ARRAY_CONTENTS.iter()).for_each(
                            |(arr_elem, expected)| {
                                assert_eq!(
                                    *arr_elem
                                        .validate(&mut access)
                                        .expect("Failed to validate array contents"),
                                    *expected
                                )
                            },
                        );
                    })
                    .expect("Error allocating [u8; 16] on foreign stack");
            },
        )
    });
}
