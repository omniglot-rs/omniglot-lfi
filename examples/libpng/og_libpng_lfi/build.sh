#!/usr/bin/env bash

set -xe

LFI_TOOLCHAIN_PATH="$1"
LFI_TOOLCHAIN_PREFIX="x86_64_lfi-linux-musl-"

HOST_TOOLCHAIN_PATH="$2"
HOST_TOOLCHAIN_PREFIX="x86_64-unknown-linux-musl-"

# apt update
# apt install -y zlib1g zlib1g-dev

function buildZlib {
    TOOLCHAIN_PREFIX="$1"
    SUFFIX="$2"

    mkdir -p "./build/zlib_${SUFFIX}"
    pushd "./build/zlib_${SUFFIX}"
    cmake \
        -D"CMAKE_C_COMPILER=${TOOLCHAIN_PREFIX}clang" \
        -D"CMAKE_CXX_COMPILER=${TOOLCHAIN_PREFIX}clang++" \
        -D"CMAKE_LINKER=${TOOLCHAIN_PREFIX}ld" \
        -DCMAKE_INSTALL_PREFIX=./install \
        ../../zlib_src
    cmake --build . --target install
    popd
}

function buildLibPNG {
    TOOLCHAIN_PREFIX="$1"
    SUFFIX="$2"
    ZLIB_INSTALL_DIR="$(realpath "$3")"

    mkdir -p "./build/libpng_${SUFFIX}"
    pushd "./build/libpng_${SUFFIX}"
    cmake \
	-D"ZLIB_LIBRARY=${ZLIB_INSTALL_DIR}/lib/libz.a" \
    	-D"ZLIB_INCLUDE_DIR=${ZLIB_INSTALL_DIR}/include" \
        -D"CMAKE_C_COMPILER=${TOOLCHAIN_PREFIX}clang" \
        -D"CMAKE_CXX_COMPILER=${TOOLCHAIN_PREFIX}clang++" \
        -D"CMAKE_LINKER=${TOOLCHAIN_PREFIX}ld" \
        -DCMAKE_INSTALL_PREFIX=./install \
        ../../libpng_src
    cmake --build . --target install
    popd
}

function buildOGLFIProgram() {
    VARIANT="$1"
    TOOLCHAIN_PREFIX="$2"
    LIBPNG_ZLIB_SUFFIX="$3"

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
    "${TOOLCHAIN_PREFIX}clang" \
        -Wall -Werror \
	-I "./build/libpng_${LIBPNG_ZLIB_SUFFIX}/install/include" \
        ${VARIANT_CFLAGS} \
        -o "./build/libpng_nojmp_${VARIANT}.o" \
        libpng_nojmp.c \
        -c -O2 -fPIC

    # Create object file archive:
    rm -f "./build/og_libpng_${VARIANT}.a"
    llvm-ar rcs "./build/og_libpng_${VARIANT}.a" \
        "./build/og_boxrt_${VARIANT}.o" \
        "./build/libpng_nojmp_${VARIANT}.o"

    # Link, wrapping memory allocation functions:
    "${TOOLCHAIN_PREFIX}clang" \
        -Wl,--whole-archive "./build/og_libpng_${VARIANT}.a" \
	-Wl,--whole-archive "./build/zlib_${LIBPNG_ZLIB_SUFFIX}/install/lib/libz.a" \
        -Wl,--whole-archive "./build/libpng_${LIBPNG_ZLIB_SUFFIX}/install/lib/libpng.a" \
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
        -o "./build/og_libpng_${VARIANT}.lfi"
}

rm -rf ./build

PATH="$LFI_TOOLCHAIN_PATH:$PATH" buildZlib "${LFI_TOOLCHAIN_PREFIX}" "lfi"
PATH="$LFI_TOOLCHAIN_PATH:$PATH" buildLibPNG "${LFI_TOOLCHAIN_PREFIX}" "lfi" "./build/zlib_lfi/install"
PATH="$LFI_TOOLCHAIN_PATH:$PATH" buildOGLFIProgram "default" "${LFI_TOOLCHAIN_PREFIX}" "lfi"
PATH="$LFI_TOOLCHAIN_PATH:$PATH" buildOGLFIProgram "auto_allow_revoke" "${LFI_TOOLCHAIN_PREFIX}" "lfi"

PATH="$HOST_TOOLCHAIN_PATH:$PATH" buildZlib "${HOST_TOOLCHAIN_PREFIX}" "native"
PATH="$HOST_TOOLCHAIN_PATH:$PATH" buildLibPNG "${HOST_TOOLCHAIN_PREFIX}" "native" "./build/zlib_native/install"

pushd ./build
tar -czvf ./og_libpng_lfi.tar.gz \
    ./og_libpng_default.lfi \
    ./og_libpng_auto_allow_revoke.lfi \
    ./zlib_lfi/install \
    ./zlib_native/install \
    ./libpng_lfi/install \
    ./libpng_native/install
popd
