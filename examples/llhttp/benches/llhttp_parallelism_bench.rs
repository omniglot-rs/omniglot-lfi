// -*- fill-column: 80; -*-

use omniglot_lfi_example_llhttp::parse_http_request;

use rayon::prelude::*;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};

pub fn criterion_benchmark(c: &mut Criterion) {
    env_logger::init();

    let mut group = c.benchmark_group("llhttp_parse");

    for threads in (0..).map(|n| 2usize.pow(n)).take(4) {
        group.bench_with_input(
            BenchmarkId::new(&format!("{}_threads", threads), threads),
            &threads,
            |b, _| {
                let pool = rayon::ThreadPoolBuilder::new()
                    .num_threads(threads)
                    .build()
                    .unwrap();

                pool.install(|| {
                    b.iter(|| {
                        rayon::iter::repeat(b"GET / HTTP/1.1\r\n\r\n")
                            .take(1024)
                            .for_each(|req| parse_http_request(req));
                    })
                });
            },
        );
    }

    group.finish();

    println!("Finished benchmarks!");
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
