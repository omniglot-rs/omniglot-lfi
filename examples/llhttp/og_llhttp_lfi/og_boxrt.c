#include <stddef.h>
#include <stdlib.h>
#include <stdatomic.h>
#include <stdbool.h>

#include "og_boxrt.h"

struct og_boxrt_state {
    _Atomic bool initialized;

    // Allow the host program to access a given region of memory.
    og_allow_cb allow_cb;

    // Revoke a previously allowed region of memory, ensuring that the
    // host program cannot access it any more.
    og_revoke_cb revoke_cb;
};

struct og_boxrt_state OG_BOXRT_STATE = { .initialized = false };

void og_boxrt_init(og_allow_cb allow_cb, og_revoke_cb revoke_cb) {
    struct og_boxrt_state *state = &OG_BOXRT_STATE;

    // We must never attempt to double-initialize the boxrt state:
    if (state->initialized) {
        abort();
    }

    // Copy the callback pointers:
    state->allow_cb = allow_cb;
    state->revoke_cb = revoke_cb;

    // Indicate that the state has been initialized:
    state->initialized = true;
}

// Wrap the following symbols:
//   - malloc
//   - free
//   - calloc
//   - realloc
//   - aligned_alloc
//   - posix_memalign
//   - memalign
//   - valloc
//
// The following symbols are not defined:
//   - pvalloc
//   - cfree

void *__real_malloc(size_t);
void *__wrap_malloc(size_t size) {
    void *start = __real_malloc(size);

#ifdef OG_BOXRT_AUTO_ALLOW_REVOKE
    if (start && OG_BOXRT_STATE.initialized && OG_BOXRT_STATE.allow_cb) {
        bool allow_res = OG_BOXRT_STATE.allow_cb(start, size, true);
	if (!allow_res) {
	    abort();
	}
    }
#endif

    return start;
}

void __real_free(void *ptr);
void __wrap_free(void *ptr) {
#ifdef OG_BOXRT_AUTO_ALLOW_REVOKE
    // Don't do anything for a null pointer:
    if (ptr && OG_BOXRT_STATE.initialized && OG_BOXRT_STATE.revoke_cb) {
	bool revoke_res = OG_BOXRT_STATE.revoke_cb(ptr);
	if (!revoke_res) {
	    abort();
	}
    }
#endif

    __real_free(ptr);
}


void *__real_calloc(size_t nmemb, size_t size);
void *__wrap_calloc(size_t nmemb, size_t size) {
    void *start = __real_calloc(nmemb, size);

#ifdef OG_BOXRT_AUTO_ALLOW_REVOKE
    if (start && OG_BOXRT_STATE.initialized && OG_BOXRT_STATE.allow_cb) {
        if (!OG_BOXRT_STATE.allow_cb(start, nmemb * size, true)) {
            abort();
        }
    }
#endif

    return start;
}

void *__real_realloc(void *ptr, size_t size);
void *__wrap_realloc(void *ptr, size_t size) {
#ifdef OG_BOXRT_AUTO_ALLOW_REVOKE
    // Revoke old pointer before resizing if it exists
    if (ptr && OG_BOXRT_STATE.initialized && OG_BOXRT_STATE.revoke_cb) {
        if (!OG_BOXRT_STATE.revoke_cb(ptr)) {
            abort();
        }
    }
#endif

    void *start = __real_realloc(ptr, size);

#ifdef OG_BOXRT_AUTO_ALLOW_REVOKE
    // Allow new pointer after resizing
    if (start && OG_BOXRT_STATE.initialized && OG_BOXRT_STATE.allow_cb) {
        if (!OG_BOXRT_STATE.allow_cb(start, size, true)) {
            abort();
        }
    }
#endif

    return start;
}

void *__real_aligned_alloc(size_t alignment, size_t size);
void *__wrap_aligned_alloc(size_t alignment, size_t size) {
    void *start = __real_aligned_alloc(alignment, size);

#ifdef OG_BOXRT_AUTO_ALLOW_REVOKE
    if (start && OG_BOXRT_STATE.initialized && OG_BOXRT_STATE.allow_cb) {
        if (!OG_BOXRT_STATE.allow_cb(start, size, true)) {
            abort();
        }
    }
#endif

    return start;
}

int __real_posix_memalign(void **memptr, size_t alignment, size_t size);
int __wrap_posix_memalign(void **memptr, size_t alignment, size_t size) {
    int result = __real_posix_memalign(memptr, alignment, size);

#ifdef OG_BOXRT_AUTO_ALLOW_REVOKE
    // posix_memalign returns 0 on success
    if (result == 0 && *memptr && OG_BOXRT_STATE.initialized && OG_BOXRT_STATE.allow_cb) {
        if (!OG_BOXRT_STATE.allow_cb(*memptr, size, true)) {
            abort();
        }
    }
#endif

    return result;
}

void *__real_memalign(size_t alignment, size_t size);
void *__wrap_memalign(size_t alignment, size_t size) {
    void *start = __real_memalign(alignment, size);

#ifdef OG_BOXRT_AUTO_ALLOW_REVOKE
    if (start && OG_BOXRT_STATE.initialized && OG_BOXRT_STATE.allow_cb) {
        if (!OG_BOXRT_STATE.allow_cb(start, size, true)) {
            abort();
        }
    }
#endif

    return start;
}

void *__real_valloc(size_t size);
void *__wrap_valloc(size_t size) {
    void *start = __real_valloc(size);

#ifdef OG_BOXRT_AUTO_ALLOW_REVOKE
    if (start && OG_BOXRT_STATE.initialized && OG_BOXRT_STATE.allow_cb) {
        if (!OG_BOXRT_STATE.allow_cb(start, size, true)) {
            abort();
        }
    }
#endif

    return start;
}
