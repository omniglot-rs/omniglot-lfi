use omniglot::rt::OGRuntime;

use omniglot_lfi_microbenchmarks::libogdemo::LibOGDemo;
use omniglot_lfi_microbenchmarks::with_lfi_sysv_amd64_rt_lib;

pub fn main() {
    env_logger::init();

    omniglot::id::lifetime::OGLifetimeBranding::new(|brand| {
        with_lfi_sysv_amd64_rt_lib(brand, |lib, mut alloc, mut access| {
            lib.rt()
                .allocate_stacked_t_mut::<bool, _, _>(&mut alloc, |bool_ref, alloc| {
                    lib.demo_write_invalid_bool(bool_ref.as_ptr(), alloc, &mut access)
                        .unwrap()
                        .validate()
                        .unwrap();
                    println!("{:?}", bool_ref.validate(&mut access).map(|val| *val));
                })
                .unwrap();
        });
    });
}
