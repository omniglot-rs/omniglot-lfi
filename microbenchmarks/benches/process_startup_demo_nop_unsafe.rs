fn main() {
    // Used to measure startup time against
    // process_startup_demo_nop_og_lfi.rs
    unsafe { omniglot_lfi_microbenchmarks::libogdemo::demo_nop() }
}
