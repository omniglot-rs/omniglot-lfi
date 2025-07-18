#include <lfi_core.h>

struct LFIInvokeInfo* og_lfi_get_threadlocal_invoke_info() {
    return &lfi_invoke_info;
}
