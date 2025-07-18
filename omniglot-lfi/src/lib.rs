#![feature(
    maybe_uninit_write_slice,
    maybe_uninit_as_bytes,
    thread_local,
)]

pub mod amd64;
pub mod common;

mod liblfi;

// use std::ffi::{CStr, c_char, c_int};
pub mod prog;

// fn lfi_init() {
//     // Create engine if it does not exist.
//     let ok: bool = unsafe {
//         liblfi::lfi_linux_lib_init(
//             liblfi::LFIOptions {
//                 boxsize: 4 * 1024 * 1024 * 1024,
//                 pagesize: page_size::get(),
//                 verbose: false,
//                 no_verify: true,

//                 // Default values:
//                 allow_wx: false,
//                 no_init_sigaltstack: false,
//                 stores_only: false,
//             },
//             liblfi::LFILinuxOptions {
//                 stacksize: 2 * 1024 * 1024,
//                 verbose: false,

//                 // Defualt values:
//                 dir_maps: std::ptr::null_mut(),
//                 exit_unknown_syscalls: false,
//                 perf: false,
//                 sys_passthrough: false,
//                 wd: std::ptr::null(),
//             },
//         )
//     };

//     if !ok {
//         panic!("error: failed to initialize liblfi");
//     }
// }

// fn resolve(proc: *mut liblfi::LFILinuxProc, loc: &mut liblfi::lfiptr, name: &CStr) {
//     let addr: liblfi::lfiptr =
//         unsafe { liblfi::lfi_proc_sym(proc, name.as_ptr()) } as liblfi::lfiptr;
//     if addr == 0 {
//         panic!("error: symbol not found: {}", name.to_string_lossy());
//     } else {
//         println!(
//             "Resolved symbol {} to addr {:p}",
//             name.to_string_lossy(),
//             addr as *mut ()
//         );
//     }
//     *loc = addr;
// }

// fn main() {
//     let mut ok;

//     println!("Hello, world!");

//     lfi_init();

//     let proc: *mut liblfi::LFILinuxProc =
//         unsafe { liblfi::lfi_proc_new(liblfi::lfi_linux_lib_engine()) };
//     if proc == std::ptr::null_mut() {
//         panic!("error: failed to create LFI proc");
//     }

//     ok = unsafe { liblfi::lfi_proc_load(proc, prog::PROG.as_ptr() as *mut _, prog::PROG.len()) };
//     if !ok {
//         panic!("error: failed to load LFI library");
//     }

//     // Initialize return.
//     unsafe { liblfi::lfi_box_init_ret(liblfi::lfi_proc_box(proc)) };

//     // Initialize callbacks.
//     ok = unsafe { liblfi::lfi_box_cbinit(liblfi::lfi_proc_box(proc)) };
//     if !ok {
//         panic!("error: failed to initialize LFI callbacks");
//     }

//     // Create and run thread.
//     let progname = c"addbox";
//     let mut argv: [*const c_char; 2] = [progname.as_ptr(), std::ptr::null()];
//     let mut envp: [*const c_char; 1] = [std::ptr::null()];

//     let t: *mut liblfi::LFILinuxThread = unsafe {
//         liblfi::lfi_thread_new(
//             proc,
//             (argv.len() / std::mem::size_of::<*const c_char>()) as i32,
//             argv.as_mut_ptr(),
//             envp.as_mut_ptr(),
//         )
//     };
//     if t == std::ptr::null_mut() {
//         panic!("failed to initialize LFI thread");
//     }

//     let result: c_int = unsafe { liblfi::lfi_thread_run(t) };
//     if result != 0 {
//         panic!("LFI thread returned non-zero value: {}", result);
//     }

//     // Initialize clone.
//     unsafe { liblfi::lfi_linux_init_clone(t) };

//     let addbox_box: *mut liblfi::LFIBox = unsafe { liblfi::lfi_proc_box(proc) };
//     let mut addbox_ctx: *mut liblfi::LFIContext = unsafe { *liblfi::lfi_thread_ctxp(t) };

//     let mut addbox_addr_add: liblfi::lfiptr = 0;
//     resolve(proc, &mut addbox_addr_add, c"add");

//     // // Initialize all exported symbols.
//     // extern lfiptr addbox_addr_add;
//     // resolve(proc, &addbox_addr_add, "add");

//     unsafe {
//         *liblfi::og_lfi_get_threadlocal_invoke_info() = liblfi::LFIInvokeInfo {
//             ctx: &mut addbox_ctx as *mut _,
//             targetfn: addbox_addr_add,
//             box_: addbox_box,
//         }
//     };

//     let add_trampoline: unsafe extern "C" fn(c_int, c_int) -> c_int =
//         unsafe { std::mem::transmute(liblfi::lfi_trampoline_addr) };

//     println!("add(1, 2) = {}", unsafe { add_trampoline(1, 2) });
// }
