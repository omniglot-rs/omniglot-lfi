#!/usr/bin/env bash

set -xe

x86_64-lfi-linux-musl-clang add.c -c -O2 -fPIC
llvm-ar rcs libadd.a add.o
x86_64-lfi-linux-musl-clang -Wl,--whole-archive libadd.a -Wl,--no-whole-archive -Wl,--export-dynamic -lboxrt -static-pie -o libadd.lfi
