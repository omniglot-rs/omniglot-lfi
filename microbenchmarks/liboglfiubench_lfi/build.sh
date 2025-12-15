#!/usr/bin/env bash

set -xe

function buildVariant() {
    VARIANT="$1"
    ALLOCATOR="$2"

    VARIANT_CFLAGS=""
    if [ "${VARIANT}" == "auto_allow_revoke" ]; then
	VARIANT_CFLAGS="${VARIANT_CFLAGS} -DOG_BOXRT_AUTO_ALLOW_REVOKE"
    fi

    # Compile source files:
    x86_64-lfi-linux-musl-clang -Wall -Werror ${VARIANT_CFLAGS} og_boxrt.c -c -O2 -fPIC
    x86_64-lfi-linux-musl-clang -Wall -Werror ${VARIANT_CFLAGS} og_lfi_ubench.c -c -O2 -fPIC

    # Create object file archive:
    rm -f "liboglfiubench_${VARIANT}.a"
    llvm-ar rcs "liboglfiubench_${VARIANT}.a" \
        og_lfi_ubench.o \
        og_boxrt.o

    # Link, wrapping memory allocation functions:
    LINK_MIMALLOC="$([[ "$ALLOCATOR" == "mimalloc" ]] && echo "-lmimalloc" || true)"
    x86_64-lfi-linux-musl-clang \
        -Wl,--whole-archive "liboglfiubench_${VARIANT}".a \
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
        -o "liboglfiubench_${VARIANT}.lfi"
}

buildVariant "musl_default" "musl"
buildVariant "mimalloc_default" "mimalloc"
buildVariant "musl_auto_allow_revoke" "musl"
buildVariant "mimalloc_auto_allow_revoke" "mimalloc"
