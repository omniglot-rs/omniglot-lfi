#!/usr/bin/env bash

set -xe

x86_64-lfi-linux-musl-clang -Wall -Werror og_lfi_tests.c -c -O2 -fPIC
llvm-ar rcs liboglfitests.a og_lfi_tests.o
x86_64-lfi-linux-musl-clang -Wl,--whole-archive liboglfitests.a -Wl,--no-whole-archive -Wl,--export-dynamic -lboxrt -static-pie -o liboglfitests.lfi
