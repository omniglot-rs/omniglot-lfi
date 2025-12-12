// -*- fill-column: 80; -*-

use std::hint::black_box;

use omniglot::rt::OGRuntime;

use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};

use criterion::{criterion_group, criterion_main};
use criterion::{BenchmarkId, Criterion, Throughput};

use omniglot_lfi_example_brotli::brotli::{Brotli, BrotliRt};
use omniglot_lfi_example_brotli::{
    test_brotli_og, test_brotli_unsafe, with_lfi_sysv_amd64_rt_lib, with_mockrt_lib,
};

// Function to randomize the host stack layout on which the to-be benchmarked
// function runs. This is to avoid influences of the stack layout influencing
// the measured library performance.
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
    assert!(STACK_RANDOMIZE_ITERS > 0);

    let mut prng = SmallRng::seed_from_u64(0xDEADBEEFCAFEBABE);

    omniglot::id::lifetime::OGLifetimeBranding::new(|lfi_brand| {
        with_lfi_sysv_amd64_rt_lib(
            lfi_brand,
            |lfi_lib: BrotliRt<_, _, _>, mut lfi_alloc, mut lfi_access| {
                omniglot::id::lifetime::OGLifetimeBranding::new(|mock_brand| {
                    with_mockrt_lib(
                        mock_brand,
                        omniglot::rt::mock::stack_alloc::StackAllocator::<
                            omniglot::rt::mock::stack_alloc::StackFrameAllocAMD64,
                        >::new(),
                        |mock_lib: BrotliRt<_, _, _>, mut mock_alloc, mut mock_access| {
                            let mut group = c.benchmark_group("brotli_compress_decompress");

                            for size in [8, 64, 128, 256, 512, 1024] {
                                let tput_bytes: u64 = size as u64;

                                group.throughput(Throughput::Bytes(tput_bytes as u64));

                                group.bench_with_input(
                                    BenchmarkId::new("unsafe", tput_bytes),
                                    &tput_bytes,
                                    |b, _| {
                                        for _ in 0..STACK_RANDOMIZE_ITERS {
                                            let stack_bytes: usize = (&mut prng).random_range(
                                                std::ops::RangeInclusive::new(1_usize, 4095_usize),
                                            );
                                            push_stack_bytes(stack_bytes, || {
                                                b.iter(|| {
                                                    black_box(unsafe {
                                                        test_brotli_unsafe(black_box(size))
                                                    });
                                                });
                                            });
                                        }
                                    },
                                );

                                group.bench_with_input(
                                    BenchmarkId::new("omniglot-lfi", tput_bytes),
                                    &tput_bytes,
                                    |b, _| {
                                        for _ in 0..STACK_RANDOMIZE_ITERS {
                                            let stack_bytes: usize = (&mut prng).random_range(
                                                std::ops::RangeInclusive::new(1_usize, 4095_usize),
                                            );
                                            let foreign_stack_bytes: usize = (&mut prng)
                                                .random_range(std::ops::RangeInclusive::new(
                                                    1_usize, 4095_usize,
                                                ));
                                            push_stack_bytes(stack_bytes, || {
                                                lfi_lib
                                                    .rt()
                                                    .allocate_stacked_mut(
                                                        std::alloc::Layout::from_size_align(
                                                            foreign_stack_bytes,
                                                            1,
                                                        )
                                                        .unwrap(),
                                                        &mut lfi_alloc,
                                                        |_, lfi_alloc| {
                                                            b.iter(|| {
                                                                black_box(test_brotli_og(
                                                                    &lfi_lib,
                                                                    lfi_alloc,
                                                                    &mut lfi_access,
                                                                    black_box(size),
                                                                ))
                                                            });
                                                        },
                                                    )
                                                    .unwrap();
                                            });
                                        }
                                    },
                                );

                                group.bench_with_input(
                                    BenchmarkId::new("omniglot-mock", tput_bytes),
                                    &tput_bytes,
                                    |b, _| {
                                        for _ in 0..STACK_RANDOMIZE_ITERS {
                                            let stack_bytes: usize = (&mut prng).random_range(
                                                std::ops::RangeInclusive::new(1_usize, 4095_usize),
                                            );
                                            let foreign_stack_bytes: usize = (&mut prng)
                                                .random_range(std::ops::RangeInclusive::new(
                                                    1_usize, 4095_usize,
                                                ));
                                            push_stack_bytes(stack_bytes, || {
                                                mock_lib
                                                    .rt()
                                                    .allocate_stacked_mut(
                                                        std::alloc::Layout::from_size_align(
                                                            foreign_stack_bytes,
                                                            1,
                                                        )
                                                        .unwrap(),
                                                        &mut mock_alloc,
                                                        |_, mock_alloc| {
                                                            b.iter(|| {
                                                                black_box(test_brotli_og(
                                                                    &mock_lib,
                                                                    mock_alloc,
                                                                    &mut mock_access,
                                                                    black_box(size),
                                                                ));
                                                            });
                                                        },
                                                    )
                                                    .unwrap();
                                            });
                                        }
                                    },
                                );
                            }
                            group.finish();
                        },
                    );
                });
            },
        );
    });

    println!("Finished benchmarks!");
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
