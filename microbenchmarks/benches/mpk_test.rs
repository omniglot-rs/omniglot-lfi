fn main() {
    env_logger::init();

    use omniglot_lfi_microbenchmarks::libogdemo::LibOGDemo;

    omniglot::id::lifetime::OGLifetimeBranding::new(|brand| {
        omniglot_lfi_microbenchmarks::with_lfi_sysv_amd64_rt_lib(
            brand,
            |lib, mut alloc, mut access| {
                const ITERS: usize = 1000_000_000;
                println!("Running {ITERS} transitions");
                for _ in 0..ITERS {
                    lib.demo_nop(&mut alloc, &mut access).unwrap();
                    // println!("Iter!");
                }
                println!("Done!");
            },
        )
    })
}
