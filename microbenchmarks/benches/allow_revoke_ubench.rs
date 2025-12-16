use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};

use criterion::{criterion_group, criterion_main, Criterion};

use omniglot::rt::OGRuntime;

use omniglot_lfi_microbenchmarks::libogdemo::LibOGDemo;
use omniglot_lfi_microbenchmarks::with_lfi_sysv_amd64_rt_lib;

const STACK_RANDOMIZE_ITERS: usize = 1;

fn push_stack_bytes<R>(bytes: usize, f: impl FnOnce() -> R) -> R {
    use omniglot::rt::mock::MockRtAllocator;
    let stack_alloc = omniglot::rt::mock::stack_alloc::StackAllocator::<
        omniglot::rt::mock::stack_alloc::StackFrameAllocAMD64,
    >::new();
    unsafe {
        stack_alloc
            .with_alloc(
                core::alloc::Layout::from_size_align(bytes, 1).unwrap(),
                |_| f(),
            )
            .map_err(|_| ())
            .unwrap()
    }
}

pub fn criterion_benchmark(c: &mut Criterion) {
    env_logger::init();

    let mut prng = SmallRng::seed_from_u64(0xDEADBEEFCAFEBABE);

    omniglot::id::lifetime::OGLifetimeBranding::new(|brand| {
        with_lfi_sysv_amd64_rt_lib(brand, |lib, mut alloc, mut access| {
            let mut group = c.benchmark_group("allow_revoke_ubench");

            group.bench_function("nop", |b| {
                for _ in 0..STACK_RANDOMIZE_ITERS {
                    let stack_bytes: usize =
                        prng.random_range(std::ops::RangeInclusive::new(1_usize, 4095_usize));
                    let foreign_stack_bytes: usize =
                        prng.random_range(std::ops::RangeInclusive::new(1_usize, 4095_usize));
                    push_stack_bytes(stack_bytes, || {
                        lib.rt()
                            .allocate_stacked_mut(
                                std::alloc::Layout::from_size_align(foreign_stack_bytes, 1)
                                    .unwrap(),
                                &mut alloc,
                                |_, alloc| {
                                    b.iter(|| lib.demo_nop(alloc, &mut access).unwrap());
                                },
                            )
                            .unwrap();
                    });
                }
            });

            group.bench_function("allow_revoke", |b| {
                const LEN: usize = 42;
                let mallocd_ptr = lib.rt().malloc(LEN, &mut alloc, &mut access).unwrap();
                let start_ptr = mallocd_ptr.wrapping_byte_add(1);

                for _ in 0..STACK_RANDOMIZE_ITERS {
                    let stack_bytes: usize =
                        prng.random_range(std::ops::RangeInclusive::new(1_usize, 4095_usize));
                    let foreign_stack_bytes: usize =
                        prng.random_range(std::ops::RangeInclusive::new(1_usize, 4095_usize));
                    push_stack_bytes(stack_bytes, || {
                        lib.rt()
                            .allocate_stacked_mut(
                                std::alloc::Layout::from_size_align(foreign_stack_bytes, 1)
                                    .unwrap(),
                                &mut alloc,
                                |_, alloc| {
                                    b.iter(|| {
                                        lib.demo_allow_revoke(
                                            start_ptr,
                                            LEN - 1,
                                            true,
                                            alloc,
                                            &mut access,
                                        )
                                        .unwrap()
                                    });
                                },
                            )
                            .unwrap();
                    });
                }

                lib.rt().free(mallocd_ptr, &mut alloc, &mut access).unwrap();
            });

            group.finish();
        });
    });

    println!("Finished benchmarks!");
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
