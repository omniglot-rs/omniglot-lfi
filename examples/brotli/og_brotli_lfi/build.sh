#!/usr/bin/env bash

set -xe

LFI_TOOLCHAIN_PREFIX=x86_64_lfi-linux-musl

rm -rf ./build
mkdir -p ./build/brotli
pushd ./build/brotli
cmake \
    -DBUILD_SHARED_LIBS=OFF \
    -D"CMAKE_C_COMPILER=${LFI_TOOLCHAIN_PREFIX}-clang" \
    -D"CMAKE_CXX_COMPILER=${LFI_TOOLCHAIN_PREFIX}-clang++" \
    -D"CMAKE_LINKER=${LFI_TOOLCHAIN_PREFIX}-ld" \
    -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_INSTALL_PREFIX=./install \
    ../../brotli_src
cmake --build . --config Release --target install
popd

function buildVariant() {
    VARIANT="$1"

    VARIANT_CFLAGS=""
    if [ "${VARIANT}" == "auto_allow_revoke" ]; then
        VARIANT_CFLAGS="${VARIANT_CFLAGS} -DOG_BOXRT_AUTO_ALLOW_REVOKE"
    fi

    # Compile source files:
    "${LFI_TOOLCHAIN_PREFIX}-clang" \
        -Wall -Werror \
        ${VARIANT_CFLAGS} \
        -o "./build/og_boxrt_${VARIANT}.o" \
        og_boxrt.c \
        -c -O2 -fPIC

    # Create object file archive:
    rm -f "./build/og_boxrt_${VARIANT}.a"
    llvm-ar rcs "./build/og_boxrt_${VARIANT}.a" \
        "./build/og_boxrt_${VARIANT}.o"

    # Link, wrapping memory allocation functions:
    "${LFI_TOOLCHAIN_PREFIX}-clang" \
        -Wl,--whole-archive "./build/og_boxrt_${VARIANT}.a" \
        -Wl,--whole-archive ./build/brotli/install/lib/libbrotlicommon.a \
        -Wl,--whole-archive ./build/brotli/install/lib/libbrotlienc.a \
        -Wl,--whole-archive ./build/brotli/install/lib/libbrotlidec.a \
        -Wl,--no-whole-archive \
        -Wl,--export-dynamic \
        -lboxrt \
        -static-pie \
        -Wl,--wrap=malloc \
        -Wl,--wrap=free \
        -Wl,--wrap=calloc \
        -Wl,--wrap=realloc \
        -Wl,--wrap=aligned_alloc \
        -Wl,--wrap=posix_memalign \
        -Wl,--wrap=memalign \
        -Wl,--wrap=valloc \
        -o "./build/og_brotli_${VARIANT}.lfi"
}

buildVariant "default"
buildVariant "auto_allow_revoke"

pushd ./build
tar -czvf ./og_brotli_lfi.tar.gz \
    ./og_brotli_default.lfi \
    ./og_brotli_auto_allow_revoke.lfi \
    ./brotli/install
popd
