#!/usr/bin/env bash

set -xe

function buildVariant() {
    VARIANT="$1"

    VARIANT_CFLAGS=""
    if [ "${VARIANT}" == "auto_allow_revoke" ]; then
	VARIANT_CFLAGS="${VARIANT_CFLAGS} -DOG_BOXRT_AUTO_ALLOW_REVOKE"
    fi

    # Compile source files:
    x86_64-lfi-linux-musl-clang -Wall -Werror ${VARIANT_CFLAGS} og_boxrt.c -c -O2 -fPIC
    x86_64-lfi-linux-musl-clang -Wall -Werror ${VARIANT_CFLAGS} og_lfi_tests.c -c -O2 -fPIC

    # Create object file archive:
    rm -f "liboglfitests_${VARIANT}.a"
    llvm-ar rcs "liboglfitests_${VARIANT}.a" \
        og_lfi_tests.o \
        og_boxrt.o

    # Link, wrapping memory allocation functions:
    x86_64-lfi-linux-musl-clang \
        -Wl,--whole-archive "liboglfitests_${VARIANT}".a \
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
        -o "liboglfitests_${VARIANT}.lfi"
}

buildVariant "default"
buildVariant "auto_allow_revoke"
