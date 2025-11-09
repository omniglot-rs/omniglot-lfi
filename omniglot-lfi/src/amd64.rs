use std::cell::UnsafeCell;
use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::marker::{PhantomData, PhantomPinned};
use std::mem::MaybeUninit;
use std::pin::Pin;

use log;

use omniglot::abi::calling_convention::{AREG0, AREG1, AREG2, AREG3, AREG4, AREG5, Stacked};
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

#[repr(C)]
pub struct OGLFISysVAMD64Runtime<ID: OGID> {
    // -------------------------------------------------------------------------
    // Misc state:
    id: ID,
    lfi_proc: *mut liblfi::LFILinuxProc,
    lfi_thread: *mut liblfi::LFILinuxThread,
    lfi_box: *mut liblfi::LFIBox,
    lfi_ctx: *mut liblfi::LFIContext,

    // Pointer to the current LFI context's LFIRegs struct, prepared by the
    // `execute` hook and valid for the single call to `invoke` within its
    // callback closure:
    invoke_lfi_regs_ptr: UnsafeCell<*mut liblfi::LFIRegs>,

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
                id,

                lfi_proc,
                lfi_thread,
                lfi_box,
                lfi_ctx,

                invoke_lfi_regs_ptr: UnsafeCell::new(std::ptr::null_mut()),

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
                // TODO: why is this is a double-pointer? Is the inner pointer
                // expected to change, and if so, when?
                ctx: &mut lfi_ctx as *mut _,
                targetfn: target_symbol as liblfi::lfiptr,
                box_: self.lfi_box,
            }
        };

        // Determine the pointer to the LFIRegs struct from the LFI context,
        // valid solely for a single function call within the provided callback
        // closure.
        //
        // # Safety
        //
        // This value is only accessed in two places, on the same thread,
        // non-concurrently: here, in this `execute` hook, and within the
        // `invoke` assembly run as part of the provided closure.
        unsafe {
            *self.invoke_lfi_regs_ptr.get() = liblfi::lfi_ctx_regs(self.lfi_ctx);
        }

        let res = f();

        // Clear the `invoke_lfi_regs_ptr` field, to detect when `invoke` is
        // incorrectly called outside of this `execute` hook's closure.
        //
        // # Safety
        //
        // This value is only accessed in two places, on the same thread,
        // non-concurrently: here, in this `execute` hook, and within the
        // `invoke` assembly run as part of the provided closure.
        unsafe {
            *self.invoke_lfi_regs_ptr.get() = std::ptr::null_mut();
        }

        Ok(res)
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

macro_rules! invoke_asm {
    ($ctx_stack_offset_src:expr, $rtptrloc:expr, $resptrloc:expr $(,)?) => {
        core::arch::naked_asm!(
            // When the context arguments are indexed by register offset
            // instead of on the stack, `ctx_stack_offset` is never used in
            // this assembly block, which raises a compiler error. Thus, use
            // it in a comment:
            "// dummy use of ctx_stack_offset: {ctx_stack_offset}",

            // Push the "invoke result" struct pointer, which we write
            // the return registers to after returning from the
            // trampoline:
            concat!("push ", $resptrloc),

            // Load the runtime struct pointer into a well-known
            // caller-saved, **non-argument** register (r10). This may
            // be a simple register--register mov, or a
            // memory-to-register copy (which is why we can't indirectly
            // address using $rtptrloc).
            //
            // We'll need it after determining the LFIRegs struct
            // address to copy part of the host stack onto the foreign
            // library, and after returning from the trampoline to reset
            // the LFI stack.
            //
            // Is it important to use a **non-argument** register here,
            // as those may contain function arguments that must be
            // preserved all the way through the call to
            // `lfi_trampoline`.
            concat!("mov r10, ", $rtptrloc),

            // We need to copy a part of the host's stack into the
            // sandbox. Before we do so, we save the current sandbox
            // stack pointer, and restore it after the sandbox returns.
            //
            // To manipulate any of the sandbox state, we need to
            // retrieve the LFIRegs pointer. The execute hook has
            // helpfully saved this pointer into the runtime struct, so
            // we can avoid a function call (and stacking any clobbered
            // registers here).
            //
            // Load the LFIRegs pointer into r10 (caller-saved,
            // non-argument) and push it onto the stack (we'll need
            // access to it to restore the original stack pointer, and
            // it may be overwritten by a nested invoke in a callback):
            "mov r10, qword ptr [r10 + {rt_invoke_lfi_regs_ptr_offset}]",
            "push r10",
            //
            // Load the current sandbox stack pointer into r11
            // (caller-saved, non-argument) and save it onto the stack:
            "mov r11, qword ptr [r10 + {lfiregs_rsp_offset}]",
            "push r11",
            //
            // Now we copy the stacked arguments from the host stack.
            // For this we need to:
            //
            // 1. Subtract bytes occupied by stacked arguments:
            "sub r11, {stack_spill}",
            //
            // 2. If subtraction underflowed (carry is set), return a
            //    stack overflow error:
            "jc 150f",
            //
            // 3. Align new stack downward to a 16-byte boundary:
            "and r11, -16",
            //
            // 4. Check for stack overflow against stack_bottom:
            "jmp 200f", // TODO: compare against a "stack_bottom"

            "150:", // ----- STACK OVERFLOW OCCURRED -----
            //
            // A stack overflow occurred; we need to report it and
            // return. Recover the "invoke result" struct pointer:
            "add rsp, 16", // pop sandbox stack ptr & *mut LFIRegs
            "pop r11", // pop "invoke result" struct pointer
            //
            // Indicate a stack overflow error:
            "mov qword ptr [r11 + {ir_error_offset}], {ie_stack_overflow_const}",
            //
            // Return, which will convert this into an Err(_) result:
            "ret",
            //
            "200:", // ----- NO STACK OVERFLOW -----
            //
            // 5. Copy `{stack_spill}` bytes from our current stack
            //    pointer to the foreign stack.
            //
            // Now that we know that the stack pointer fits
            // `{stack_size}` (we've moved it down by that amount), and
            // we've aligned it, we can copy `{stack_size}` to it.
            //
            // We use the x86 "rep movsq" string-copy instruction
            // sequence, which is supposed to be fast even for large
            // copies on recent microarchitectures, and avoids us having
            // to reason about padding the amount to copy to full
            // quadwords. It also avoids using a shadow-copy of the rsp
            // registers, which we can't arbitrarily move outside the
            // red-zone during the copy operation for signal handling
            // safety.
            //
            // It does however require us to clobber some argument
            // registers, which we place on the stack:
            "push rsi",
            "push rdi",
            "push rcx",
            //
            // Perform the copy. We offset into the stack pointer by 56
            // bytes (7 quadwords):
            //
            // - 8 bytes stacked rcx,
            // - 8 bytes stacked rdi,
            // - 8 bytes stacked rsi,
            // - 8 bytes stacked original sandbox stack pointer,
            // - 8 bytes stacked LFIRegs pointer,
            // - 8 bytes stacked "return result" struct pointer,
            // - 8 bytes stacked return address,
            //
            "lea rsi, [rsp + 56]",     // Source
            "mov rdi, r11",           // Destination
            "mov rcx, {stack_spill}", // Length
            "cld",                    // DF = 0, incrementing copy
            "rep movsb",              // Copy until rcx is 0
            //
            // Unstack temporarily stacked registers:
            "pop rcx",
            "pop rdi",
            "pop rsi",

            // Finally, save the updated stack pointer back to the
            // LFIRegs struct:
            "mov qword ptr [r10 + {lfiregs_rsp_offset}], r11",

            // Call the LFI trampoline. The function to invoke has been
            // configured just before running this function, as part of
            // the Runtime's `execute` hook.
            //
            // The stack is properly aligned for this function call at
            // this point. We entered `invoke` with a half-aligned
            // stack after a call instruction and pushed 4 quadwords,
            // making our stack 16-byte aligned.
            "call lfi_trampoline",

            // Restore the original sandbox pointer.
            //
            // First, pop the original sandbox stack pointer itself:
            "pop r11",
            //
            // Then, pop the LFIRegs pointer:
            "pop r10",
            //
            // Write the original sandbox stack pointer to LFIRegs:
            "mov qword ptr [r10 + {lfiregs_rsp_offset}], r11",

            // Pop the InvokeRes struct pointer from the stack
            // (overwriting the original sandbox stack pointer, which we
            // no longer need):
            "pop r11",

            // Store the function's return value registers. This is
            // irrespective of whether both registers are used,
            // initialized, or the function even returned properly. We
            // save them in `MaybeUninit`s and later determine how to
            // interpret them, based on whether LFI indicated a
            // successful function return and the function's signature.
            "mov qword ptr [r11 + {ir_error_offset}], {ie_no_error_const}",
            "mov qword ptr [r11 + {ir_rax_offset}], rax", // rax return value
            "mov qword ptr [r11 + {ir_rdx_offset}], rdx", // rdx return value

            // Finally, return to the function-specific wrapper, which
            // will perform the return-value encoding:
            "ret",

            // Amount of bytes to copy to the sandbox stack:
            stack_spill = const STACK_SPILL,

            // Where the context arguments are located (runtime struct
            // pointer, etc.), relative to the stack pointer (if they
            // are stacked):
            ctx_stack_offset = const $ctx_stack_offset_src,

            // Runtime struct offsets:
            rt_invoke_lfi_regs_ptr_offset = const std::mem::offset_of!(
                OGLFISysVAMD64Runtime<ID>, invoke_lfi_regs_ptr),

            // LFIRegs struct offsets:
            lfiregs_rsp_offset = const std::mem::offset_of!(liblfi::LFIRegs, rsp),

            // InvokeResInner struct offsets:
            ir_error_offset = const std::mem::offset_of!(OGLFISysVAMD64InvokeResInner, error),
            ir_rax_offset = const std::mem::offset_of!(OGLFISysVAMD64InvokeResInner, rax),
            ir_rdx_offset = const std::mem::offset_of!(OGLFISysVAMD64InvokeResInner, rdx),

            // InvokeResError constants:
            ie_no_error_const = const OGLFISysVAMD64InvokeErr::NoError as usize,
            ie_stack_overflow_const = const OGLFISysVAMD64InvokeErr::StackOverflow as usize,
        )
    }

}

macro_rules! invoke_impl_asm_register_ctx {
    ($ctxreg:ident, $rtptrloc:expr, $resptrloc:expr $(,)?) => {
        impl<const STACK_SPILL: usize, ID: OGID> SysVAMD64Rt<STACK_SPILL, $ctxreg<SysVAMD64ABI>>
            for OGLFISysVAMD64Runtime<ID>
        {
            #[unsafe(naked)]
            unsafe extern "C" fn invoke() {
                invoke_asm!(0, $rtptrloc, $resptrloc);
            }
        }
    };
}

invoke_impl_asm_register_ctx!(AREG0, "rdi", "rdx");
invoke_impl_asm_register_ctx!(AREG1, "rsi", "rcx");
invoke_impl_asm_register_ctx!(AREG2, "rdx", "r8");
invoke_impl_asm_register_ctx!(AREG3, "rcx", "r9");
invoke_impl_asm_register_ctx!(AREG4, "r8", "qword ptr [rsp + 8]");
invoke_impl_asm_register_ctx!(AREG5, "r9", "qword ptr [rsp + 16]");

impl<const STACK_SPILL: usize, const CTX_STACK_OFFSET: usize, ID: OGID>
    SysVAMD64Rt<STACK_SPILL, Stacked<{ CTX_STACK_OFFSET }, SysVAMD64ABI>>
    for OGLFISysVAMD64Runtime<ID>
{
    #[unsafe(naked)]
    unsafe extern "C" fn invoke() {
        invoke_asm!(
            CTX_STACK_OFFSET,
            // This "runtime struct" pointer is offset by 8 more bytes than it
            // should be, because it is loaded after a "push" instruction to
            // save the "return struct" pointer:
            "qword ptr [rsp + {ctx_stack_offset} + 16]",
            // Return struct pointer, at the correct address:
            "qword ptr [rsp + {ctx_stack_offset} + 24]",
        );
    }
}

impl<ID: OGID> SysVAMD64BaseRt for OGLFISysVAMD64Runtime<ID> {
    type InvokeRes<T> = OGLFISysVAMD64InvokeRes<Self, T>;
}

#[repr(usize)]
enum OGLFISysVAMD64InvokeErr {
    NoError,
    NotCalled,
    StackOverflow,
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

            OGLFISysVAMD64InvokeErr::StackOverflow => Err(OGError::StackOverflow),

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
