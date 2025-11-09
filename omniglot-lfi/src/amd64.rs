use std::cell::UnsafeCell;
use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::marker::{PhantomData, PhantomPinned};
use std::mem::MaybeUninit;
use std::pin::Pin;

use log;

use omniglot::abi::calling_convention::{AREG0, AREG1, AREG2, AREG3, AREG4, AREG5};
use omniglot::abi::sysv_amd64::SysVAMD64ABI;
use omniglot::foreign_memory::og_copy::OGCopy;
use omniglot::id::OGID;
use omniglot::markers::{AccessScope, AllocScope};
use omniglot::rt::sysv_amd64::{SysVAMD64BaseRt, SysVAMD64InvokeRes, SysVAMD64Rt};
use omniglot::rt::{CallbackContext, CallbackReturn, OGRuntime};
use omniglot::{OGError, OGResult};

use crate::common::OGLFIAllocTracker;
use crate::liblfi;

struct ForcePin<T> {
    inner: T,
    _pin: PhantomPinned,
}

impl<T> ForcePin<T> {
    pub fn new(inner: T) -> Self {
        ForcePin {
            inner,
            _pin: PhantomPinned,
        }
    }
}

impl<T> std::ops::Deref for ForcePin<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

#[thread_local]
static mut RUNTIME_THREAD_STATE: OGLFISysVAMD64RuntimeThreadState =
    OGLFISysVAMD64RuntimeThreadState::new();

struct OGLFISysVAMD64RuntimeThreadState {
    runtime: *const (),
}

impl OGLFISysVAMD64RuntimeThreadState {
    const fn new() -> Self {
        OGLFISysVAMD64RuntimeThreadState {
            runtime: std::ptr::null(),
        }
    }
}

#[repr(C)]
pub struct OGLFISysVAMD64Runtime<ID: OGID> {
    // -------------------------------------------------------------------------
    // Temporary values saved and restored in the `invoke` trampolines:
    //
    // Scratch-space to store the InvokeRes pointer for encoding the function's
    // return value while executing foreign code:
    invoke_res_ptr: UnsafeCell<*mut OGLFISysVAMD64InvokeResInner>,

    // -------------------------------------------------------------------------
    // Misc state:
    id: ID,
    lfi_proc: *mut liblfi::LFILinuxProc,
    lfi_thread: *mut liblfi::LFILinuxThread,
    lfi_box: *mut liblfi::LFIBox,
    lfi_ctx: *mut liblfi::LFIContext,

    // -------------------------------------------------------------------------
    // Pinned objects, pointers to which we pass into the sandbox and
    // thus should live for as long as the sandbox does:
    pinned_program_name: Pin<Box<ForcePin<CString>>>,
    pinned_arguments: Vec<Pin<Box<ForcePin<CString>>>>,
    pinned_argv: Pin<Box<ForcePin<Vec<*const c_char>>>>,
    pinned_envp: Pin<Box<ForcePin<Vec<*const c_char>>>>,
}

impl<ID: OGID> OGLFISysVAMD64Runtime<ID> {
    pub fn from_lfi_lib_bytes(
        lfi_library: &[u8],
        program_name: CString,
        arguments: impl Iterator<Item = CString>,
        id: ID,
    ) -> OGResult<(
        Self,
        AllocScope<'static, <Self as OGRuntime>::AllocTracker<'static>, ID>,
        AccessScope<ID>,
    )> {
        log::debug!(
            "Creating new LFI sandbox for LFI library of {} bytes with program name {:?}",
            lfi_library.len(),
            &program_name
        );

        // Instantiate the LFI engine, if one does not exist.
        let lfi_linux_lib_init_res: bool = unsafe {
            liblfi::lfi_linux_lib_init(
                liblfi::LFIOptions {
                    boxsize: 4 * 1024 * 1024 * 1024,
                    pagesize: page_size::get(),
                    verbose: false,
                    no_verify: true,

                    // Don't need compatibility with old rewriters,
                    // this will become "true" in the future by
                    // default:
                    no_rtcall_nullpage: false,

                    // Default values:
                    allow_wx: false,
                    no_init_sigaltstack: false,
                    stores_only: false,
                },
                liblfi::LFILinuxOptions {
                    stacksize: 2 * 1024 * 1024,
                    verbose: false,
                    debug: false,

                    // Default values:
                    dir_maps: std::ptr::null_mut(),
                    exit_unknown_syscalls: false,
                    perf: false,
                    sys_passthrough: false,
                    wd: std::ptr::null(),
                    brk_control: false,
                    brk_size: 0,
                },
            )
        };
        if !lfi_linux_lib_init_res {
            log::error!("Failed to initialize liblfi engine");
            return Err(OGError::InternalError);
        }
        log::trace!("Initialized liblfi engine");

        let lfi_proc: *mut liblfi::LFILinuxProc =
            unsafe { liblfi::lfi_proc_new(liblfi::lfi_linux_lib_engine()) };
        if lfi_proc == std::ptr::null_mut() {
            log::error!("Failed to create LFI proc");
            return Err(OGError::InternalError);
        }
        log::trace!("Created LFI process: {:p}", lfi_proc);

        let pinned_program_name = Box::pin(ForcePin::new(program_name));
        let lfi_proc_load_res = unsafe {
            liblfi::lfi_proc_load(
                lfi_proc,
                lfi_library.as_ptr() as *mut _,
                lfi_library.len(),
                pinned_program_name.as_ref().as_ptr(),
            )
        };
        if !lfi_proc_load_res {
            log::error!("Failed to load LFI library");
            return Err(OGError::InternalError);
        }

        // Initialize return and callbacks:
        unsafe { liblfi::lfi_box_init_ret(liblfi::lfi_proc_box(lfi_proc)) };
        let lfi_box_cbinit_res = unsafe { liblfi::lfi_box_cbinit(liblfi::lfi_proc_box(lfi_proc)) };
        if !lfi_box_cbinit_res {
            log::error!("Failed to initialize LFI callbacks");
            return Err(OGError::InternalError);
        }

        let pinned_arguments = arguments
            .map(|arg| Box::pin(ForcePin::new(arg)))
            .collect::<Vec<_>>();

        let mut argv: Vec<*const c_char> = Vec::with_capacity(1 + pinned_arguments.len());
        argv.push(pinned_program_name.as_ref().as_ptr());
        for arg in &pinned_arguments {
            argv.push(arg.as_ref().as_ptr());
        }
        argv.push(std::ptr::null());

        let pinned_argv = Box::pin(ForcePin::new(argv));
        let pinned_envp: Pin<Box<ForcePin<Vec<*const c_char>>>> =
            Box::pin(ForcePin::new(vec![std::ptr::null()]));

        let lfi_thread: *mut liblfi::LFILinuxThread = unsafe {
            liblfi::lfi_thread_new(
                lfi_proc,
                (pinned_argv.as_ref().len() / std::mem::size_of::<*const c_char>()) as i32,
                pinned_argv.as_ref().as_ptr() as *mut _,
                pinned_envp.as_ref().as_ptr() as *mut _,
            )
        };
        if lfi_thread == std::ptr::null_mut() {
            log::error!("Failed to initialize LFI thread");
            return Err(OGError::InternalError);
        }

        let lfi_thread_run_res: c_int = unsafe { liblfi::lfi_thread_run(lfi_thread) };
        if lfi_thread_run_res != 0 {
            panic!(
                "lfi_thread_run(lfi_thread = {:p}) returned non-zero value: {}",
                lfi_thread, lfi_thread_run_res
            );
        }

        // Initialize clone.
        unsafe { liblfi::lfi_linux_init_clone(lfi_thread) };

        let lfi_box: *mut liblfi::LFIBox = unsafe { liblfi::lfi_proc_box(lfi_proc) };
        let lfi_ctx: *mut liblfi::LFIContext = unsafe { *liblfi::lfi_thread_ctxp(lfi_thread) };

        let id_imprint = id.get_imprint();

        Ok((
            OGLFISysVAMD64Runtime {
                invoke_res_ptr: UnsafeCell::new(std::ptr::null_mut()),

                id,

                lfi_proc,
                lfi_thread,
                lfi_box,
                lfi_ctx,

                pinned_program_name,
                pinned_arguments,
                pinned_argv,
                pinned_envp,
            },
            unsafe { AllocScope::new(OGLFIAllocTracker, id_imprint) },
            unsafe { AccessScope::new(id_imprint) },
        ))
    }
}

unsafe impl<ID: OGID> OGRuntime for OGLFISysVAMD64Runtime<ID> {
    type ID = ID;
    type ABI = SysVAMD64ABI;

    type AllocTracker<'a> = OGLFIAllocTracker;

    type CallbackTrampolineFn = ();
    type CallbackContext = OGLFISysVAMD64CallbackContext;
    type CallbackReturn = OGLFISysVAMD64CallbackReturn;

    type SymbolTableState<const SYMTAB_SIZE: usize, const FIXED_OFFSET_SYMTAB_SIZE: usize> =
        OGLFISysVAMD64SymbolTable<SYMTAB_SIZE>;

    fn resolve_symbols<const SYMTAB_SIZE: usize, const FIXED_OFFSET_SYMTAB_SIZE: usize>(
        &self,
        compact_symbol_table: &'static [&'static CStr; SYMTAB_SIZE],
        _fixed_offset_symbol_table: &'static [Option<&'static CStr>; FIXED_OFFSET_SYMTAB_SIZE],
    ) -> Result<Self::SymbolTableState<SYMTAB_SIZE, FIXED_OFFSET_SYMTAB_SIZE>, Option<&'static CStr>>
    {
        let mut missing_symbol = None;

        // We clone the fixed-size array reference passed above and map on it,
        // which allows us to avoid using a temporary heap-allocation (possibly
        // at the expense of high stack usage):
        let symbols = compact_symbol_table.clone().map(|symbol_name| {
            if missing_symbol.is_some() {
                // If we error on one symbol, don't need to loop up others.
                std::ptr::null()
            } else {
                let addr: liblfi::lfiptr =
                    unsafe { liblfi::lfi_proc_sym(self.lfi_proc, symbol_name.as_ptr()) }
                        as liblfi::lfiptr;

                // Check if the lookup succeeded:
                if addr != std::ptr::null::<()>() as liblfi::lfiptr {
                    // Success! Found the symbol.

                    // We stuff an lfiptr into a *const (). Make sure it fits!
                    const _: () = assert!(
                        std::mem::size_of::<liblfi::lfiptr>() <= std::mem::size_of::<*const ()>()
                    );

                    // Cast the pointer, proceed to the next symbol:
                    let sym = addr as *const ();

                    log::debug!(
                        "Resolved LFI box symbol with name \"{}\" = {:p}",
                        symbol_name.to_string_lossy(),
                        sym
                    );
                    sym
                } else {
                    // Did not find a library that exposes this symbol:
                    log::debug!(
                        "Failed to resolve LFI box symbol with name \"{}\"",
                        symbol_name.to_string_lossy()
                    );

                    missing_symbol = Some(symbol_name);
                    std::ptr::null_mut()
                }
            }
        });

        if let Some(s) = missing_symbol {
            Err(Some(s))
        } else {
            Ok(OGLFISysVAMD64SymbolTable { symbols })
        }
    }

    fn lookup_symbol<const SYMTAB_SIZE: usize, const FIXED_OFFSET_SYMTAB_SIZE: usize>(
        &self,
        compact_symtab_index: usize,
        _fixed_offset_symtab_index: usize,
        symtabstate: &Self::SymbolTableState<SYMTAB_SIZE, FIXED_OFFSET_SYMTAB_SIZE>,
    ) -> Option<*const ()> {
        symtabstate.symbols.get(compact_symtab_index).copied()
    }

    fn allocate_stacked_untracked_mut<F, R>(
        &self,
        _requested_layout: core::alloc::Layout,
        _fun: F,
    ) -> OGResult<R>
    where
        F: FnOnce(*mut ()) -> R,
    {
        todo!()
    }

    fn allocate_stacked_mut<'a, F, R>(
        &self,
        _layout: core::alloc::Layout,
        _alloc_scope: &mut AllocScope<'_, Self::AllocTracker<'_>, ID>,
        _fun: F,
    ) -> Result<R, OGError>
    where
        F: for<'b> FnOnce(*mut (), &'b mut AllocScope<'_, Self::AllocTracker<'_>, Self::ID>) -> R,
    {
        todo!()
    }

    fn setup_callback<'a, C, F, R>(
        &self,
        _callback: &'a mut C,
        _alloc_scope: &mut AllocScope<'_, Self::AllocTracker<'_>, Self::ID>,
        _fun: F,
    ) -> OGResult<R>
    where
        C: FnMut(
            &Self::CallbackContext,
            &mut Self::CallbackReturn,
            &mut AllocScope<'_, Self::AllocTracker<'_>, Self::ID>,
            &mut AccessScope<Self::ID>,
        ),
        F: for<'b> FnOnce(
            *const Self::CallbackTrampolineFn,
            &'b mut AllocScope<'_, Self::AllocTracker<'_>, Self::ID>,
        ) -> R,
    {
        todo!()
    }

    fn execute<R, F: FnOnce() -> R>(
        &self,
        target_symbol: *const (),
        _alloc_scope: &mut AllocScope<'_, Self::AllocTracker<'_>, Self::ID>,
        _access_scope: &mut AccessScope<Self::ID>,
        f: F,
    ) -> OGResult<R> {
        let mut lfi_ctx = self.lfi_ctx;

        unsafe {
            *liblfi::og_lfi_get_threadlocal_invoke_info() = liblfi::LFIInvokeInfo {
                ctx: &mut lfi_ctx as *mut _,
                targetfn: target_symbol as liblfi::lfiptr,
                box_: self.lfi_box,
            }
        };

        Ok(f())
    }
}

pub struct OGLFISysVAMD64SymbolTable<const SYMTAB_SIZE: usize> {
    symbols: [*const (); SYMTAB_SIZE],
}

#[derive(Debug, Clone, Copy)]
pub struct OGLFISysVAMD64CallbackContext;
impl CallbackContext for OGLFISysVAMD64CallbackContext {
    fn get_argument_register(&self, _: usize) -> Option<usize> {
        todo!()
    }

    fn get_stack_pointer(&self) -> *mut c_void {
        todo!()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct OGLFISysVAMD64CallbackReturn;
impl CallbackReturn for OGLFISysVAMD64CallbackReturn {
    fn set_return_register(&mut self, _: usize, _: usize) -> bool {
        todo!()
    }
}

macro_rules! invoke_impl_rtloc_register {
    ($regtype:ident, $rtloc:expr, $fnptrloc:expr, $resptrloc:expr) => {
        impl<const STACK_SPILL: usize, ID: OGID>
            SysVAMD64Rt<STACK_SPILL, $regtype<SysVAMD64ABI>>
            for OGLFISysVAMD64Runtime<ID>
        {
            #[unsafe(naked)]
            unsafe extern "C" fn invoke() {
                core::arch::naked_asm!(
		    // First, save the invoke res pointer into the runtime
		    // struct:
		    concat!("mov qword ptr [", $rtloc, " + {rt_invoke_res_ptr_offset}], ", $resptrloc),

		    // Then, save the runtime pointer into the
		    // RuntimeThreadState thread-local:
                    concat!("mov qword ptr fs:[{rths_static_sym}@TPOFF + {rths_runtime_offset}], ", $rtloc),

		    // Call the LFI trampoline. The function to invoke has been
		    // configured just before running this function, as part of
		    // the Runtime's `execute` hook:
		    "call lfi_trampoline",

		    // Recover the runtime struct pointer from the
		    // RuntimeThreadState thread-local and save it into a
		    // caller-saved register, `r10`:
		    "mov r10, qword ptr fs:[{rths_static_sym}@TPOFF + {rths_runtime_offset}]",

		    // Recover the InvokeRes struct pointer from the runtime
		    // struct, and save it into a caller-saved register `r12`:
		    "mov r12, qword ptr [r10 + {rt_invoke_res_ptr_offset}]",

		    // Store the function's return value registers. This is
		    // irrespective of whether both registers are used,
		    // initialized, or the function even returned properly. We
		    // save them in `MaybeUninit`s and later determine how to
		    // interpret them, based on whether LFI indicated a
		    // successful function return and the function's signature.
		    "mov qword ptr [r12 + {ir_error_offset}], {ie_no_error_const}",
                    "mov qword ptr [r12 + {ir_rax_offset}], rax", // rax return value
	            "mov qword ptr [r12 + {ir_rdx_offset}], rdx", // rdx return value

		    // Finally, return to the function-specific wrapper, which
		    // will perform the return-value encoding:
		    "ret",

		    // Runtime struct offsets:
		    rt_invoke_res_ptr_offset = const std::mem::offset_of!(Self, invoke_res_ptr),

		    // RuntimeThreadState symbol and struct offsets:
		    rths_static_sym = sym RUNTIME_THREAD_STATE,
		    rths_runtime_offset = const std::mem::offset_of!(OGLFISysVAMD64RuntimeThreadState, runtime),

		    // InvokeResInner struct offsets:
		    ir_error_offset = const std::mem::offset_of!(OGLFISysVAMD64InvokeResInner, error),
		    ir_rax_offset = const std::mem::offset_of!(OGLFISysVAMD64InvokeResInner, rax),
		    ir_rdx_offset = const std::mem::offset_of!(OGLFISysVAMD64InvokeResInner, rdx),

		    // InvokeResError constants:
		    ie_no_error_const = const OGLFISysVAMD64InvokeErr::NoError as usize,
               );
            }
        }
    };
}

invoke_impl_rtloc_register!(AREG0, "rdi", "rsi", "rdx");
invoke_impl_rtloc_register!(AREG1, "rsi", "rdx", "rcx");
invoke_impl_rtloc_register!(AREG2, "rdx", "rcx", "r8");
invoke_impl_rtloc_register!(AREG3, "rcx", "r8", "r9");
invoke_impl_rtloc_register!(AREG4, "r8", "r9", "[rsp + 8]");
invoke_impl_rtloc_register!(AREG5, "r9", "[rsp + 8]", "[rsp + 16]");

// impl<const STACK_SPILL: usize, const RT_STACK_OFFSET: usize, ID: OGID>
//     SysVAMD64Rt<STACK_SPILL, Stacked<RT_STACK_OFFSET, SysVAMD64ABI>> for OGLFISysVAMD64Runtime<ID>
// {
//     #[unsafe(naked)]
//     unsafe extern "C" fn invoke() {
//         core::arch::naked_asm!(
// 	    "ud2",
//             // "
//             // // This pushes the stack down by {pushed} bytes. We rely on this
//             // // offset below. ALWAYS UPDATE THEM IN TANDEM.
//             // push rbx
//             // push rbp
//             // push r12
//             // push r13
//             // push r14
//             // push r15
//             // // BEFORE CHANGING THE ABOVE, DID YOU READ THE COMMENT?

//             // // Load required parameters in non-argument registers and
//             // // continue execution in the generic protection-domain
//             // // switch routine:
//             // mov r10, [rsp + {pushed} + {rt_stack_offset} + 8]  // Load runtime pointer into r10 from stack offset + 8
//             // mov r11, [rsp + {pushed} + {rt_stack_offset} + 16] // Load function pointer into r11 from stack offset + 16
//             // mov r12, [rsp + {pushed} + {rt_stack_offset} + 24] // Load the InvokeRes pointer into r12 from stack offset + 24
//             // mov r13, {stack_spill}                            // Copy the stack-spill immediate into r13
//             // lea r14, [rip - {invoke_fn}]
//             // jmp r14
//             // ",
//             // stack_spill = const STACK_SPILL,
//             // rt_stack_offset = const RT_STACK_OFFSET,
//             // invoke_fn = sym Self::generic_invoke,
//             // // How many bytes we pushed onto the stack above. This value is also used in
//             // // generic_invoke. When updating this value, ALSO UPDATE IT IN GENERIC INVOKE.
//             // pushed = const 48,
//         );
//     }
// }

impl<ID: OGID> SysVAMD64BaseRt for OGLFISysVAMD64Runtime<ID> {
    type InvokeRes<T> = OGLFISysVAMD64InvokeRes<Self, T>;
}

#[repr(usize)]
enum OGLFISysVAMD64InvokeErr {
    NoError,
    NotCalled,
}

// Depending on the size of the return value, it will be either passed
// as a pointer on the stack as the first argument, or be written to
// %rax and %rdx. In either case, this InvokeRes type is passed by
// reference (potentially on the stack), such that we can even encode
// values that exceed the available two return registers. If a return
// value was passed by invisible reference, we will be passed a
// pointer to that:
#[repr(C)]
pub struct OGLFISysVAMD64InvokeResInner {
    error: OGLFISysVAMD64InvokeErr,
    rax: usize,
    rdx: usize,
}

#[repr(C)]
pub struct OGLFISysVAMD64InvokeRes<RT: SysVAMD64BaseRt, T> {
    inner: OGLFISysVAMD64InvokeResInner,
    _t: PhantomData<T>,
    _rt: PhantomData<RT>,
}

impl<RT: SysVAMD64BaseRt, T> OGLFISysVAMD64InvokeRes<RT, T> {
    fn encode_eferror(&self) -> Result<(), OGError> {
        match self.inner.error {
            OGLFISysVAMD64InvokeErr::NotCalled => panic!(
                "Attempted to use / query {} without it being used by an invoke call!",
                std::any::type_name::<Self>()
            ),

            OGLFISysVAMD64InvokeErr::NoError => Ok(()),
        }
    }
}

unsafe impl<RT: SysVAMD64BaseRt, T> SysVAMD64InvokeRes<RT, T> for OGLFISysVAMD64InvokeRes<RT, T> {
    fn new() -> Self {
        // Required invariant by our assembly:
        let _: () = assert!(std::mem::offset_of!(Self, inner) == 0);

        OGLFISysVAMD64InvokeRes {
            inner: OGLFISysVAMD64InvokeResInner {
                error: OGLFISysVAMD64InvokeErr::NotCalled,
                rax: 0,
                rdx: 0,
            },
            _t: PhantomData,
            _rt: PhantomData,
        }
    }

    fn into_result_registers(self, _rt: &RT) -> OGResult<OGCopy<T>> {
        self.encode_eferror()?;

        // Basic assumptions in this method:
        // - sizeof(usize) == sizeof(u64)
        // - little endian
        assert!(std::mem::size_of::<usize>() == std::mem::size_of::<u64>());
        assert!(cfg!(target_endian = "little"));

        // This function must not be called on types larger than two
        // pointers (128 bit), as those cannot possibly be encoded in the
        // two available 64-bit return registers:
        assert!(std::mem::size_of::<T>() <= 2 * std::mem::size_of::<*const ()>());

        // Allocate space to construct the final (unvalidated) T from
        // the register values. During copy, we treat the memory of T
        // as integers:
        let mut ret_uninit: MaybeUninit<T> = MaybeUninit::uninit();

        // TODO: currently, we only support power-of-two return values.
        // It is not immediately obvious how values that are, e.g.,
        // 9 byte in size would be encoded into registers.
        let rax_bytes = u64::to_le_bytes(self.inner.rax as u64);
        let rdx_bytes = u64::to_le_bytes(self.inner.rdx as u64);
        let ret_bytes = [
            rax_bytes[0],
            rax_bytes[1],
            rax_bytes[2],
            rax_bytes[3],
            rax_bytes[4],
            rax_bytes[5],
            rax_bytes[6],
            rax_bytes[7],
            rdx_bytes[0],
            rdx_bytes[1],
            rdx_bytes[2],
            rdx_bytes[3],
            rdx_bytes[4],
            rdx_bytes[5],
            rdx_bytes[6],
            rdx_bytes[7],
        ];

        ret_uninit
            .as_bytes_mut()
            .write_copy_of_slice(&ret_bytes[..std::mem::size_of::<T>()]);

        OGResult::Ok(ret_uninit.into())
    }

    unsafe fn into_result_stacked(self, _rt: &RT, stacked_res: *mut T) -> OGResult<OGCopy<T>> {
        self.encode_eferror()?;

        // Allocate space to construct the final (unvalidated) T from
        // the register values. During copy, we treat the memory of T
        // as integers:
        let mut ret_uninit: MaybeUninit<T> = MaybeUninit::uninit();

        // Now, we simply do a memcpy from our pointer. We trust the caller that
        // the provided pointer is allocated, not aliasing any Rust struct, not
        // being mutated concurrently, and accessible to us. We cast it into a
        // layout-compatible MaybeUninit pointer:
        unsafe {
            std::ptr::copy_nonoverlapping(stacked_res as *const T, ret_uninit.as_mut_ptr(), 1)
        };

        OGResult::Ok(ret_uninit.into())
    }
}
