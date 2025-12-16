#include <stddef.h>
#include <stdlib.h>
#include <stdatomic.h>
#include <stdbool.h>

#include "og_boxrt.h"

void og_boxrt_allow(void *start, size_t size, bool mutable) {}
void og_boxrt_revoke(void *ptr) {}
