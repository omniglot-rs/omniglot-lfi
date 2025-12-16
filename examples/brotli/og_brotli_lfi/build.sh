#!/usr/bin/env bash

set -xe

LFI_TOOLCHAIN_PATH="$1"
LFI_TOOLCHAIN_PREFIX="x86_64_lfi-linux-musl-"

HOST_TOOLCHAIN_PATH="$2"
HOST_TOOLCHAIN_PREFIX="x86_64-unknown-linux-musl-"

function buildBrotli {
    TOOLCHAIN_PREFIX="$1"
    SUFFIX="$2"

    mkdir -p "./build/brotli_${SUFFIX}"
    pushd "./build/brotli_${SUFFIX}"
    cmake \
        -DBUILD_SHARED_LIBS=OFF \
        -D"CMAKE_C_COMPILER=${TOOLCHAIN_PREFIX}clang" \
        -D"CMAKE_CXX_COMPILER=${TOOLCHAIN_PREFIX}clang++" \
        -D"CMAKE_LINKER=${TOOLCHAIN_PREFIX}ld" \
        -DCMAKE_BUILD_TYPE=Release \
        -DCMAKE_INSTALL_PREFIX=./install \
        ../../brotli_src
    cmake --build . --config Release --target install
    popd
}

function buildOGLFIProgram() {
    VARIANT="$1"
    TOOLCHAIN_PREFIX="$2"
    BROTLI_SUFFIX="$3"
    ALLOCATOR="$4"

    VARIANT_CFLAGS=""
    # If the variant ends with "auto_allow_revoke", enable the feature:
    if [[ "${VARIANT}" =~ ^.*auto_allow_revoke$ ]]; then
        VARIANT_CFLAGS="${VARIANT_CFLAGS} -DOG_BOXRT_AUTO_ALLOW_REVOKE"
    fi

    # Compile source files:
    "${TOOLCHAIN_PREFIX}clang" \
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
    LINK_MIMALLOC="$([[ "$ALLOCATOR" == "mimalloc" ]] && echo "-lmimalloc" || true)"
    "${TOOLCHAIN_PREFIX}clang" \
        -Wl,--whole-archive "./build/og_boxrt_${VARIANT}.a" \
        -Wl,--whole-archive "./build/brotli_${BROTLI_SUFFIX}/install/lib/libbrotlicommon.a" \
        -Wl,--whole-archive "./build/brotli_${BROTLI_SUFFIX}/install/lib/libbrotlienc.a" \
        -Wl,--whole-archive "./build/brotli_${BROTLI_SUFFIX}/install/lib/libbrotlidec.a" \
        -Wl,--no-whole-archive \
        -Wl,--export-dynamic \
	"$LINK_MIMALLOC" \
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

rm -rf ./build

PATH="$LFI_TOOLCHAIN_PATH:$PATH" buildBrotli "${LFI_TOOLCHAIN_PREFIX}" "lfi"
PATH="$LFI_TOOLCHAIN_PATH:$PATH" buildOGLFIProgram "musl_default" "${LFI_TOOLCHAIN_PREFIX}" "lfi" "musl"
PATH="$LFI_TOOLCHAIN_PATH:$PATH" buildOGLFIProgram "mimalloc_default" "${LFI_TOOLCHAIN_PREFIX}" "lfi" "mimalloc"
PATH="$LFI_TOOLCHAIN_PATH:$PATH" buildOGLFIProgram "musl_auto_allow_revoke" "${LFI_TOOLCHAIN_PREFIX}" "lfi" "musl"
PATH="$LFI_TOOLCHAIN_PATH:$PATH" buildOGLFIProgram "mimalloc_auto_allow_revoke" "${LFI_TOOLCHAIN_PREFIX}" "lfi" "mimalloc"

PATH="$HOST_TOOLCHAIN_PATH:$PATH" buildBrotli "${HOST_TOOLCHAIN_PREFIX}" "native"

pushd ./build
tar -czvf ./og_brotli_lfi.tar.gz \
    ./og_brotli_musl_default.lfi \
    ./og_brotli_mimalloc_default.lfi \
    ./og_brotli_musl_auto_allow_revoke.lfi \
    ./og_brotli_mimalloc_auto_allow_revoke.lfi \
    ./brotli_lfi/install \
    ./brotli_native/install
popd
