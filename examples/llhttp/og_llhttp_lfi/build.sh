#!/usr/bin/env bash

set -xe

LFI_TOOLCHAIN_PATH="$1"
LFI_TOOLCHAIN_PREFIX="x86_64_lfi-linux-musl-"

HOST_TOOLCHAIN_PATH="$2"
HOST_TOOLCHAIN_PREFIX="x86_64-unknown-linux-musl-"

function buildLlhttp {
    TOOLCHAIN_PREFIX="$1"
    SUFFIX="$2"

    mkdir -p "./build/llhttp_${SUFFIX}"
    pushd "./build/llhttp_${SUFFIX}"
    cmake \
        -DBUILD_SHARED_LIBS=OFF \
	-DBUILD_STATIC_LIBS=ON \
        -D"CMAKE_C_COMPILER=${TOOLCHAIN_PREFIX}clang" \
        -D"CMAKE_CXX_COMPILER=${TOOLCHAIN_PREFIX}clang++" \
        -D"CMAKE_LINKER=${TOOLCHAIN_PREFIX}ld" \
        -DCMAKE_BUILD_TYPE=Release \
        -DCMAKE_INSTALL_PREFIX=./install \
        ../../llhttp_src
    make
    mkdir -p ./install/lib
    cp ./libllhttp.a ./install/lib
    cp -rf ../../llhttp_src/include ./install/include
    popd
}

function buildOGLFIProgram() {
    VARIANT="$1"
    TOOLCHAIN_PREFIX="$2"
    LLHTTP_SUFFIX="$3"
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
        -Wl,--whole-archive "./build/llhttp_${LLHTTP_SUFFIX}/install/lib/libllhttp.a" \
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
        -o "./build/og_llhttp_${VARIANT}.lfi"
}

rm -rf ./build

PATH="$LFI_TOOLCHAIN_PATH:$PATH" buildLlhttp "${LFI_TOOLCHAIN_PREFIX}" "lfi"
PATH="$LFI_TOOLCHAIN_PATH:$PATH" buildOGLFIProgram "musl_default" "${LFI_TOOLCHAIN_PREFIX}" "lfi" "musl"
PATH="$LFI_TOOLCHAIN_PATH:$PATH" buildOGLFIProgram "mimalloc_default" "${LFI_TOOLCHAIN_PREFIX}" "lfi" "mimalloc"
PATH="$LFI_TOOLCHAIN_PATH:$PATH" buildOGLFIProgram "musl_auto_allow_revoke" "${LFI_TOOLCHAIN_PREFIX}" "lfi" "musl"
PATH="$LFI_TOOLCHAIN_PATH:$PATH" buildOGLFIProgram "mimalloc_auto_allow_revoke" "${LFI_TOOLCHAIN_PREFIX}" "lfi" "mimalloc"

PATH="$HOST_TOOLCHAIN_PATH:$PATH" buildLlhttp "${HOST_TOOLCHAIN_PREFIX}" "native"

pushd ./build
tar -czvf ./og_llhttp_lfi.tar.gz \
    ./og_llhttp_musl_default.lfi \
    ./og_llhttp_mimalloc_default.lfi \
    ./og_llhttp_musl_auto_allow_revoke.lfi \
    ./og_llhttp_mimalloc_auto_allow_revoke.lfi \
    ./llhttp_lfi/install \
    ./llhttp_native/install
popd
