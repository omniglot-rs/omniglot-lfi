// -*- fill-column: 80; -*-

// TODO: this should ensure that we're not seeting "Callback #1 post-panic..."
// in the output, which would indicate that we're not re-rasing the panic object
// after returning from LFI.
#[test]
#[should_panic(expected = "Inner callback panic!")]
fn test_nested_callback_panic() {
    omniglot::id::lifetime::OGLifetimeBranding::new(|brand| {
        crate::with_lfi_sysv_amd64_rt_lib::<crate::OGLFITestDefaultConfig, _, _>(
            brand,
            |lib, mut alloc, mut access| {
                use crate::liboglfitests::LibOGLFITests;
                use omniglot::rt::OGRuntime;

                println!("In Rust code");
                lib.rt()
                    .setup_callback(
                        &mut |_, _, alloc, access| {
                            println!("In Rust callback #1");
                            lib.rt()
                                .setup_callback(
                                    &mut |_, _, _, _| {
                                        println!("In Rust callback #2, about to panic!");
                                        panic!("Inner callback panic!");
                                    },
                                    alloc,
                                    |cb2_ptr, alloc| {
                                        println!("Invoking callback #2...");
                                        lib.invoke_callback(
                                            unsafe {
                                                core::mem::transmute::<
                                                    _,
                                                    Option<unsafe extern "C" fn(i32) -> i32>,
                                                >(
                                                    cb2_ptr
                                                )
                                            },
                                            1337,
                                            alloc,
                                            access,
                                        )
                                        .expect("Error executing invoke_callback function");
                                        println!("Callback #1 post-panic...");
                                    },
                                )
                                .expect("Error setting up callback #2");
                        },
                        &mut alloc,
                        |cb1_ptr, alloc| {
                            println!("Invoking callback #1...");
                            lib.invoke_callback(
                                unsafe {
                                    core::mem::transmute::<
                                        _,
                                        Option<unsafe extern "C" fn(i32) -> i32>,
                                    >(cb1_ptr)
                                },
                                42,
                                alloc,
                                &mut access,
                            )
                            .expect("Error executing invoke_callback function");
                        },
                    )
                    .expect("Error setting up callback #2");
            },
        )
    });
}
