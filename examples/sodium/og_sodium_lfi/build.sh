#!/usr/bin/env bash

set -xe

LFI_TOOLCHAIN_PATH="$1"
LFI_TOOLCHAIN_PREFIX="x86_64_lfi-linux-musl-"

HOST_TOOLCHAIN_PATH="$2"
HOST_TOOLCHAIN_PREFIX="x86_64-unknown-linux-musl-"

function buildSodium {
    TOOLCHAIN_PREFIX="$1"
    SUFFIX="$2"
    HOST_TUPLE="$3"

    mkdir -p "./build/sodium_${SUFFIX}"
    pushd "./build/sodium_${SUFFIX}"
    # We build sodium with `--disable-ssp`, as otherwise running with
    # LFI results in triggering `__stack_chk_fail`:
    ../../libsodium_src/configure \
	--prefix="$(realpath ./install)" \
	--host="${HOST_TUPLE}" \
	--disable-ssp \
	CC=${TOOLCHAIN_PREFIX}clang
    make install
    popd
}

function buildOGLFIProgram() {
    VARIANT="$1"
    TOOLCHAIN_PREFIX="$2"
    SODIUM_SUFFIX="$3"
    ALLOCATOR="$4"

    VARIANT_CFLAGS=""
    if [ "${VARIANT}" == "auto_allow_revoke" ]; then
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
        -Wl,--whole-archive "./build/sodium_${SODIUM_SUFFIX}/install/lib/libsodium.a" \
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
        -o "./build/og_sodium_${VARIANT}.lfi"
}

rm -rf ./build

PATH="$LFI_TOOLCHAIN_PATH:$PATH" buildSodium "${LFI_TOOLCHAIN_PREFIX}" "lfi" "x86_64-lfi-linux-musl"
PATH="$LFI_TOOLCHAIN_PATH:$PATH" buildOGLFIProgram "musl_default" "${LFI_TOOLCHAIN_PREFIX}" "lfi" "musl"
PATH="$LFI_TOOLCHAIN_PATH:$PATH" buildOGLFIProgram "mimalloc_default" "${LFI_TOOLCHAIN_PREFIX}" "lfi" "mimalloc"
PATH="$LFI_TOOLCHAIN_PATH:$PATH" buildOGLFIProgram "musl_auto_allow_revoke" "${LFI_TOOLCHAIN_PREFIX}" "lfi" "musl"
PATH="$LFI_TOOLCHAIN_PATH:$PATH" buildOGLFIProgram "mimalloc_auto_allow_revoke" "${LFI_TOOLCHAIN_PREFIX}" "lfi" "mimalloc"

PATH="$HOST_TOOLCHAIN_PATH:$PATH" buildSodium "${HOST_TOOLCHAIN_PREFIX}" "native" "x86_64-unknown-linux-musl"

pushd ./build
tar -czvf ./og_sodium_lfi.tar.gz \
    ./og_sodium_musl_default.lfi \
    ./og_sodium_mimalloc_default.lfi \
    ./og_sodium_musl_auto_allow_revoke.lfi \
    ./og_sodium_mimalloc_auto_allow_revoke.lfi \
    ./sodium_lfi/install \
    ./sodium_native/install
popd
