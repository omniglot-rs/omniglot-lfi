// -*- fill-column: 80; -*-

use omniglot::id::OGID;
use omniglot::markers::{AccessScope, AllocScope};
use omniglot::rt::OGRuntime;

// Auto-generated bindings, so doesn't follow Rust conventions at all:
#[allow(non_upper_case_globals)]
#[allow(non_snake_case)]
#[allow(non_camel_case_types)]
#[allow(dead_code)]
pub mod brotli {
    include!(concat!(env!("OUT_DIR"), "/brotli_bindings.rs"));
}

use brotli::{Brotli, BrotliRt};

const SOURCE_TEXT: &'static str = include_str!("../vanity_fair.txt");

pub fn test_brotli_og<ID: OGID, RT: OGRuntime<ID = ID>, L: Brotli<ID, RT, RT = RT>>(
    lib: &L,
    alloc: &mut AllocScope<RT::AllocTracker<'_>, RT::ID>,
    access: &mut AccessScope<RT::ID>,
    message_len: usize,
) {
    // Take a nicer, power-of-two number of the first characters to compress:
    let message_to_compress = SOURCE_TEXT.get(..message_len).unwrap();

    // Allocate a compressed buffer with twice the message size. This should
    // hopefully be sufficient even for entirely random messages, with any
    // headers that are attached:
    let encoded_buf_size = message_to_compress.as_bytes().len() * 2;

    lib.rt()
        .allocate_stacked_slice_mut::<u8, _, _>(encoded_buf_size, alloc, |encoded_buf, alloc| {
            let encoded_size = lib
                .rt()
                .allocate_stacked_t_mut::<usize, _, _>(alloc, |encoded_size_ref, alloc| {
                    // Before compression, the encoded size pointer argument
                    // needs to contain the available buffer space:
                    encoded_size_ref.write(encoded_buf_size, access);

                    // Copy the message into foreign memory:
                    lib.rt()
                        .allocate_stacked_slice_mut::<u8, _, _>(
                            message_to_compress.as_bytes().len(),
                            alloc,
                            |source_buf, alloc| {
                                source_buf.copy_from_slice(message_to_compress.as_bytes(), access);
                                // This will make the string invalid UTF-8,
                                // causing the below validation to fail:
                                //message_ref.write_from_iter(core::iter::repeat(0xFF), access);

                                assert_eq!(
                                    1,
                                    lib.BrotliEncoderCompress(
                                        brotli::BROTLI_DEFAULT_QUALITY as i32,
                                        brotli::BROTLI_DEFAULT_WINDOW as i32,
                                        brotli::BrotliEncoderMode_BROTLI_MODE_GENERIC,
                                        message_to_compress.as_bytes().len(),
                                        source_buf.as_ptr(),
                                        encoded_size_ref.as_ptr().into(),
                                        encoded_buf.as_ptr(),
                                        alloc,
                                        access,
                                    )
                                    .unwrap()
                                    .validate()
                                    .unwrap()
                                );
                            },
                        )
                        .unwrap();

                    // Return the encoded size:
                    *encoded_size_ref.validate(access).unwrap()
                })
                .unwrap();

            // Allocate a buffer for the decoded text, with the same length as
            // the original message.
            lib.rt()
                .allocate_stacked_slice_mut::<u8, _, _>(
                    message_to_compress.as_bytes().len(),
                    alloc,
                    |decoded_buf, alloc| {
                        // Allocate a field to store the decoded size in. It
                        // should be set to the initial available buffer space:
                        lib.rt()
                            .allocate_stacked_t_mut::<usize, _, _>(
                                alloc,
                                |decoded_size_ref, alloc| {
                                    decoded_size_ref
                                        .write(message_to_compress.as_bytes().len(), access);

                                    assert_eq!(
                                        brotli::BrotliDecoderResult_BROTLI_DECODER_RESULT_SUCCESS,
                                        lib.BrotliDecoderDecompress(
                                            encoded_size,
                                            encoded_buf.as_ptr(),
                                            decoded_size_ref.as_ptr().into(),
                                            decoded_buf.as_ptr().into(),
                                            alloc,
                                            access
                                        )
                                        .unwrap()
                                        .validate()
                                        .unwrap(),
                                    );
                                },
                            )
                            .unwrap();

                        // Compare the encoded & decoded message:
                        assert_eq!(
                            message_to_compress,
                            str::from_utf8(&*decoded_buf.as_immut().valid(access)).unwrap(),
                        );
                    },
                )
                .unwrap();
        })
        .unwrap();
}

pub unsafe fn test_brotli_unsafe(message_len: usize) {
    use std::mem::MaybeUninit;

    // Take a nicer, power-of-two number of the first characters to compress:
    let message_to_compress = SOURCE_TEXT.get(..message_len).unwrap();

    // Allocate a compressed buffer with twice the maximum message size. This
    // should hopefully be sufficient even for entirely random messages, with
    // any headers that are attached:
    let mut encoded_buf: MaybeUninit<[u8; SOURCE_TEXT.len() * 2]> = MaybeUninit::uninit();

    // Allocate a buffer for the decompressed output:
    let mut decoded_buf: MaybeUninit<[u8; SOURCE_TEXT.len()]> = MaybeUninit::uninit();

    // Before compression, the encoded size pointer argument needs to contain
    // the available buffer space:
    let mut encoded_size: usize = SOURCE_TEXT.len() * 2;

    assert_eq!(1, unsafe {
        brotli::BrotliEncoderCompress(
            brotli::BROTLI_DEFAULT_QUALITY as i32,
            brotli::BROTLI_DEFAULT_WINDOW as i32,
            brotli::BrotliEncoderMode_BROTLI_MODE_GENERIC,
            message_to_compress.as_bytes().len(),
            message_to_compress.as_bytes().as_ptr(),
            &mut encoded_size as *mut _,
            encoded_buf.as_mut_ptr() as *mut _,
        )
    },);

    // Before decompression, the decoded size pointer argument needs to contain
    // the available buffer space:
    let mut decoded_size = SOURCE_TEXT.len();

    assert_eq!(
        brotli::BrotliDecoderResult_BROTLI_DECODER_RESULT_SUCCESS,
        unsafe {
            brotli::BrotliDecoderDecompress(
                encoded_size,
                encoded_buf.as_ptr() as *mut _,
                &mut decoded_size as *mut _,
                decoded_buf.as_mut_ptr() as *mut _,
            )
        },
    );

    let decoded_buf = decoded_buf.assume_init();

    // Compare the encoded & decoded message:
    assert_eq!(message_to_compress, unsafe {
        std::str::from_utf8_unchecked(&decoded_buf[..decoded_size])
    },);
}

pub fn with_mockrt_lib<'a, ID: OGID + 'a, A: omniglot::rt::mock::MockRtAllocator, R>(
    brand: ID,
    allocator: A,
    f: impl FnOnce(
        BrotliRt<ID, omniglot::rt::mock::MockRt<ID, A>, omniglot::rt::mock::MockRt<ID, A>>,
        AllocScope<
            <omniglot::rt::mock::MockRt<ID, A> as omniglot::rt::OGRuntime>::AllocTracker<'a>,
            ID,
        >,
        AccessScope<ID>,
    ) -> R,
) -> R {
    // This is unsafe, as it instantiates a runtime that can be used to run
    // foreign functions without memory protection:
    let (rt, alloc, access) =
        unsafe { omniglot::rt::mock::MockRt::new(false, false, allocator, brand) };

    // Create a "bound" runtime, which implements the Brotli API:
    let bound_rt = BrotliRt::new(rt).unwrap();

    // Run the provided closure:
    f(bound_rt, alloc, access)
}

pub fn with_lfi_sysv_amd64_rt_lib<ID: OGID, R>(
    brand: ID,
    f: impl for<'a> FnOnce(
        BrotliRt<ID, omniglot_lfi::amd64::OGLFISysVAMD64Runtime<ID>, omniglot_lfi::amd64::OGLFISysVAMD64Runtime<ID>>,
        AllocScope<
            <omniglot_lfi::amd64::OGLFISysVAMD64Runtime<ID> as omniglot::rt::OGRuntime>::AllocTracker<'a>,
            ID,
        >,
        AccessScope<ID>,
    ) -> R,
) -> R {
    let (rt, alloc, access) = omniglot_lfi::amd64::OGLFISysVAMD64Runtime::from_lfi_lib_bytes(
        include_bytes!(concat!(
            env!("OG_BROTLI_LFI_BUILD_PATH"),
            "/og_brotli_mimalloc_default.lfi"
        )),
        c"brotli".into(),
        [].into_iter(),
        // Don't expose allow/revoke callbacks to the foreign library:
        false,
        brand,
    )
    .unwrap();

    // Create a "bound" runtime, which implements the Brotli API:
    let bound_rt = BrotliRt::new(rt)
        .expect("Failed to create bound runtime, likely problem with symbol resolution!");

    // Run the provided closure:
    f(bound_rt, alloc, access)
}
