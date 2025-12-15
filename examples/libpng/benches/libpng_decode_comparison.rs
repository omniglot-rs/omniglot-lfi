// TODO:
#![allow(static_mut_refs)]

use omniglot::rt::OGRuntime;

use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

use omniglot_lfi_example_libpng::libpng_bindings::LibPng;
use omniglot_lfi_example_libpng::{og_lfi, unsafe_ffi};

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

    // Fetch test images:
    let test_images_base_path = std::env::temp_dir().join("og_libpng_test_images");
    let test_images_path = test_images_base_path.join("images");

    fetch_prebuilt::fetch_prebuilt(
        // Archive name:
        "og_libpng_test_images",
        // Archive path environment variable. If this is set, then the
        // archive will be copied from this path, otherwise it will be
        // fetched from the URL specified in the manifest:
        "OG_LIBPNG_TEST_IMAGES_ARCHIVE",
        // Target path, to unpack the archive at:
        &test_images_path,
        // Path to store the checksum of the unpacked archive, used for
        // caching purposes:
        &test_images_base_path.join("archive_sha256.txt"),
        // Archive manifest, with URL and checksum to download from:
        &fetch_prebuilt::ArchiveManifest {
            url: "https://github.com/omniglot-rs/omniglot-lfi/releases/download/\
		  libpng-example-test-images-20251212-144325-9a18801bdfb835/\
		  omniglot-libpng-test-images.tar.gz"
                .to_string(),
            sha256: hex_literal::hex!(
                "d6866cacc70094484fb3fff6edb6f211d51846884fae9950233be5a75b64e9cd"
            )
            .to_vec(),
        },
    );

    let mut test_images: Vec<(String, Vec<u8>, (usize, usize, usize))> =
        std::fs::read_dir(test_images_path)
            .unwrap()
            .filter_map(|dir_entry_res| {
                let dir_entry = dir_entry_res.unwrap();
                if dir_entry.file_type().unwrap().is_file()
                    && dir_entry
                        .path()
                        .extension()
                        .is_some_and(|ext| ext.to_ascii_lowercase() == "png")
                {
                    Some((
                        dir_entry.file_name().into_string().unwrap(),
                        std::fs::read(dir_entry.path()).unwrap(),
                        (0, 0, 0),
                    ))
                } else {
                    None
                }
            })
            .collect();

    // Get the decompressed image size (rows, col_bytes, buffer_size):
    test_images.iter_mut().for_each(|(_, png_image, dims)| {
        // Intialize the unsafe PNG library to determine the output buffer size:
        unsafe { unsafe_ffi::png_init().unwrap() };

        // Get the image dimensions:
        let d = unsafe { unsafe_ffi::get_decompressed_image_buffer_size(png_image) };
        *dims = d;

        // Reset the library:
        unsafe { unsafe_ffi::png_destroy() };
    });

    test_images.sort_by_key(|(_, _, (_, _, buffer_size))| *buffer_size);

    println!("Loaded test image dataset:");
    for (label, bytes, (rows, cols, buffer_size)) in &test_images {
        println!(
            "- {}: {}x{}px, {}b compressed, {}b decoded",
            label,
            rows,
            cols,
            bytes.len(),
            buffer_size
        );
    }
    assert!(test_images.len() >= 1);

    // Avoid measuring large allocation overheads & heap fragmentation, compute
    // & allocate the largest target buffer once:
    let max_buffer_size: usize = test_images
        .iter()
        .map(|(_, _, (_, _, buffer_size))| *buffer_size)
        .max()
        .unwrap();

    const STACK_RANDOMIZE_ITERS: usize = 3;
    assert!(STACK_RANDOMIZE_ITERS > 0);

    let mut prng = SmallRng::seed_from_u64(0xDEADBEEFCAFEBABE);

    omniglot::id::lifetime::OGLifetimeBranding::new(|brand| {
        og_lfi::with_lfi_sysv_amd64_rt_lib(brand, |lib, mut alloc, mut access| {
            let mut group = c.benchmark_group("libpng_decode");

            // Allocate a buffer in the OG LFI domain:
            println!("running malloc!");
            let og_lfi_dst_buffer: *mut u8 = lib
                .rt()
                .malloc(max_buffer_size, &mut alloc, &mut access)
                .unwrap() as *mut u8;
            assert!(og_lfi_dst_buffer as usize % std::mem::align_of::<*mut u8>() == 0);
            println!("ran malloc, ptr: {:p}", og_lfi_dst_buffer);

            let mut unsafe_dst_buffer =
                vec![0; max_buffer_size.div_ceil(std::mem::size_of::<usize>())];

            for (test_label, png_image, (_rows, _cols, buffer_size)) in &test_images {
                let tput_bytes: u64 = *buffer_size as u64;

                group.throughput(Throughput::Bytes(tput_bytes as u64));

                group.bench_with_input(
                    BenchmarkId::new("unsafe", test_label),
                    &tput_bytes,
                    |b, _| {
                        for _ in 0..STACK_RANDOMIZE_ITERS {
                            let stack_bytes: usize = (&mut prng)
                                .random_range(std::ops::RangeInclusive::new(1_usize, 4095_usize));
                            push_stack_bytes(stack_bytes, || {
                                b.iter(|| {
                                    unsafe { unsafe_ffi::png_init().unwrap() };

                                    unsafe {
                                        unsafe_ffi::decode_png_preallocated(
                                            png_image,
                                            &mut unsafe_dst_buffer,
                                        )
                                    };

                                    unsafe { unsafe_ffi::png_destroy() };
                                });
                            });
                        }
                    },
                );

                group.bench_with_input(
                    BenchmarkId::new("og_lfi", test_label),
                    &tput_bytes,
                    |b, _| {
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
                                                let (png_ptr, info_ptr) =
                                                    og_lfi::libpng_init(&lib, alloc, &mut access);

                                                og_lfi::decode_png(
                                                    &lib,
                                                    alloc,
                                                    &mut access,
                                                    png_ptr,
                                                    info_ptr,
                                                    png_image,
                                                    Some((og_lfi_dst_buffer, max_buffer_size)),
                                                    |_, _, _, _, _| (),
                                                );

                                                og_lfi::libpng_destroy(
                                                    &lib,
                                                    alloc,
                                                    &mut access,
                                                    png_ptr,
                                                    info_ptr,
                                                );
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
        });
    });

    println!("Finished benchmarks!");
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
