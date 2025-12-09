// -*- fill-column: 80; -*-

macro_rules! add_args_test {
    ($count:expr, $expected_result:expr) => {
        paste::paste! {
            #[test]
            fn [<test_add $count>]() {
                omniglot::id::lifetime::OGLifetimeBranding::new(|brand| {
                    crate::with_lfi_sysv_amd64_rt_lib::<crate::OGLFITestDefaultConfig, _, _>(brand, |lib, mut alloc, mut access| {
                        use crate::liboglfitests::LibOGLFITests;

                        const FN_NAME: &str = stringify!([<add $count>]);

                        seq_macro::seq!(N in 1..=$count {
                            let add_res = lib.[<add $count>](
                                #(
                                    N,
                                )*
                                &mut alloc,
                                &mut access,
                            ).expect(&format!(
                                "Error executing {} function",
                                FN_NAME,
                            ));

                            assert_eq!(
                                add_res.validate().expect(&format!(
                                    "Error validating {} result",
                                    FN_NAME,
                                )),
                                $expected_result,
                            );
                        });
                    })
                });
            }
        }
    };
}

add_args_test!(0, 0);
add_args_test!(1, 1);
add_args_test!(2, 3);
add_args_test!(3, 6);
add_args_test!(4, 10);
add_args_test!(5, 15);
add_args_test!(6, 21);
add_args_test!(7, 28);
add_args_test!(8, 36);
add_args_test!(9, 45);
