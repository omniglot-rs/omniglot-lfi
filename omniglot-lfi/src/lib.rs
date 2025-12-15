// -*- fill-column: 80; -*-

#![feature(maybe_uninit_write_slice, maybe_uninit_as_bytes)]

pub mod amd64;
pub mod common;

mod liblfi;

#[derive(Copy, Clone, Debug)]
pub struct OGLFIMemoryAccessConfig {
    pub expose_boxrt_allow_revoke: bool,
    pub enable_all_sandbox_memory_access: bool,
    pub enable_sandbox_stack_access: bool,
    pub enable_allowed_memory_access: bool,
}

impl OGLFIMemoryAccessConfig {
    pub const ALL_MEMORY_ACCESSIBLE: Self = OGLFIMemoryAccessConfig {
        expose_boxrt_allow_revoke: true,
        enable_all_sandbox_memory_access: true,
        enable_sandbox_stack_access: true,
        enable_allowed_memory_access: true,
    };

    pub const STACK_OR_REQUIRE_ALLOW_REVOKE: Self = OGLFIMemoryAccessConfig {
        expose_boxrt_allow_revoke: true,
        enable_all_sandbox_memory_access: false,
        enable_sandbox_stack_access: true,
        enable_allowed_memory_access: true,
    };
}
