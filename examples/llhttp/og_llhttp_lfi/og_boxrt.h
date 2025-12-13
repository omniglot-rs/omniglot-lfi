#include <stddef.h>
#include <stdbool.h>

typedef bool (*og_allow_cb)(void *start, size_t size, bool mutable);
typedef bool (*og_revoke_cb)(void *start);

void og_boxrt_init(og_allow_cb allow_cb, og_revoke_cb revoke_cb);
