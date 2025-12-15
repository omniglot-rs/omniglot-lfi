fn main() {
    // Used to measure startup time against
    // process_startup_demo_nop_unsafe.rs
    omniglot::id::lifetime::OGLifetimeBranding::new(|brand| {
        omniglot_lfi_microbenchmarks::with_lfi_sysv_amd64_rt_lib(
            brand,
            |lib, mut alloc, mut access| {
                use omniglot_lfi_microbenchmarks::libogdemo::LibOGDemo;
                lib.demo_nop(&mut alloc, &mut access).unwrap();
            },
        );
    });
}
