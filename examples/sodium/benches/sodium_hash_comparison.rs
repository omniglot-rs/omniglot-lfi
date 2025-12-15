// -*- fill-column: 80; -*-

use std::hint::black_box;

use omniglot::rt::OGRuntime;

use omniglot_lfi_example_sodium::sodium_bindings::{self, Sodium};
use omniglot_lfi_example_sodium::{sodium_hash_og, sodium_hash_unsafe, with_lfi_sysv_amd64_rt_lib};

use rand::distr::Uniform;
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

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

    const STACK_RANDOMIZE_ITERS: usize = 3;

    let mut prng = SmallRng::seed_from_u64(0xDEADBEEFCAFEBABE);

    // Make sure the library is initialized. The MockRt and LFIRt closures do
    // this internally:
    assert!(unsafe { sodium_bindings::sodium_init() } >= 0);

    omniglot::id::lifetime::OGLifetimeBranding::new(|brand| {
        with_lfi_sysv_amd64_rt_lib(brand, |lib, mut alloc, mut access| {
            let mut group = c.benchmark_group("libsodium_hash");

            // Benchmark just the configurations of the OSDI'25 paper:
            for size in [32768] {
                // for size in (0..).map(|n| 8usize.pow(n)).skip(2).take(4) {
                let to_hash = (&mut prng)
                    .sample_iter(Uniform::new_inclusive(u8::MIN, u8::MAX).unwrap())
                    .take(size)
                    .collect::<Vec<u8>>();

                // Verify that the functions work:
                let res_unsafe = sodium_hash_unsafe(&to_hash);
                sodium_hash_og(&lib, &mut alloc, &mut access, &to_hash, |res_og| {
                    println!("{:x?}", res_unsafe);
                    assert!(&res_unsafe == res_og);
                });

                group.throughput(Throughput::Bytes(size as u64));

                group.bench_with_input(BenchmarkId::new("unsafe", size), &size, |b, _| {
                    for _ in 0..STACK_RANDOMIZE_ITERS {
                        let stack_bytes: usize = (&mut prng)
                            .random_range(std::ops::RangeInclusive::new(1_usize, 4095_usize));
                        push_stack_bytes(stack_bytes, || {
                            b.iter(|| sodium_hash_unsafe(black_box(&to_hash)));
                        });
                    }
                });

                group.bench_with_input(BenchmarkId::new("og_lfi", size), &size, |b, _| {
                    for _ in 0..STACK_RANDOMIZE_ITERS {
                        let stack_bytes: usize = (&mut prng)
                            .random_range(std::ops::RangeInclusive::new(1_usize, 4095_usize));
                        let foreign_stack_bytes: usize = (&mut prng)
                            .random_range(std::ops::RangeInclusive::new(1_usize, 4095_usize));
                        push_stack_bytes(stack_bytes, || {
                            lib.rt()
                                .allocate_stacked_mut(
                                    std::alloc::Layout::from_size_align(foreign_stack_bytes, 1)
                                        .unwrap(),
                                    &mut alloc,
                                    |_, alloc| {
                                        b.iter(|| {
                                            sodium_hash_og(
                                                &lib,
                                                alloc,
                                                &mut access,
                                                black_box(&to_hash),
                                                |_| (),
                                            )
                                        });
                                    },
                                )
                                .unwrap();
                        });
                    }
                });
            }
            group.finish();
        });
    });

    println!("Finished benchmarks!");
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
