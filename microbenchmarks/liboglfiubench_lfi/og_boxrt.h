#include <stddef.h>
#include <stdbool.h>

// Available to the library, used to allow certain memory to the Omniglot host:
void og_boxrt_allow(void *start, size_t size, bool mutable);

// Available to the library, used to revoke certain memory to the Omniglot host:
void og_boxrt_revoke(void *ptr);
