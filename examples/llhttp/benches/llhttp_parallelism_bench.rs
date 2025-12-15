// -*- fill-column: 80; -*-

use omniglot_lfi_example_llhttp::parse_http_request;

use rayon::prelude::*;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};

pub fn criterion_benchmark(c: &mut Criterion) {
    env_logger::init();

    let mut group = c.benchmark_group("llhttp_parse");

    for threads in 1..=16 {
        println!("Creating rayon ThreadPool with {threads} threads");

        // This needs to run outside of `bench_with_input`, because that runs
        // the provided closure multiple times, creating a bunch of different
        // threads, and eventally exhausting virtual / physical memory up to the
        // point where we can't create any more LFI engines (as we don't
        // implement destructors yet). Also, creating new engines is unnecessary:
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap();

        // Make sure we have enough elements in our pool to not have contention
        // over the last few elements in the chain have outsized statistical
        // influence:
        const PARSE_ITERS: usize = 64 * 1024;

        group.bench_with_input(
            BenchmarkId::new(&format!("llhttp_parse_{}req_threads", PARSE_ITERS), threads),
            &threads,
            |b, _| {
                pool.install(|| {
                    b.iter(|| {
                        rayon::iter::repeat(include_bytes!("../get_wikipedia_org_req.txt"))
                            .take(PARSE_ITERS)
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
