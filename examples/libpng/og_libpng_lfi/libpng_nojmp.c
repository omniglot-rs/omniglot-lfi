#include "libpng_nojmp.h"
#include "og_boxrt.h"

// -----------------------------------------------------------------------------
// Wrappers around png_* functions that convert setjmp error handling into
// boolean return values:

bool png_read_info_nojmp(png_structrp png_ptr, png_inforp info_ptr) {
  if (0 != setjmp(png_jmpbuf(png_ptr))) {
    return false;
  }

  png_read_info(png_ptr, info_ptr);

  return true;
}

bool png_read_image_nojmp(png_structrp png_ptr, png_bytepp image) {
  if (0 != setjmp(png_jmpbuf(png_ptr))) {
    return false;
  }

  png_read_image(png_ptr, image);

  return true;
}

// -----------------------------------------------------------------------------
// Wrapper around `png_set_read_fn` which performs an `og_boxrt_allow` on the
// buffer space that's supposed to be written to by the host, and an
// `og_boxrt_revoke` afterwards.
//
// Stores the proper callback and its user data on
// the heap:

typedef struct {
    png_voidp io_ptr;
    png_rw_ptr read_data_fn;
} png_read_fn_data_wrapped_t;

void png_read_fn_allow_revoke_wrapper(png_structp png, png_bytep ptr, size_t size) {
    // First, allow the host access to the memory:
    og_boxrt_allow(ptr, size, true);

    // Retrieve the I/O state:
    png_read_fn_data_wrapped_t *wrapped = png_get_io_ptr(png);

    // Restore the original I/O state. This can only be done by re-setting the
    // read_fn, which we set to ourselves (albeit with the original state):
    png_set_read_fn(png, wrapped->io_ptr, png_read_fn_allow_revoke_wrapper);

    // Now, run the original callback:
    wrapped->read_data_fn(png, ptr, size);

    // Restore our own I/O state, saving any potential I/O state changes by the
    // callback.
    wrapped->io_ptr = png_get_io_ptr(png);
    png_set_read_fn(png, wrapped, png_read_fn_allow_revoke_wrapper);

    // Finally, revoke access to the memory:
    og_boxrt_revoke(ptr);
}

void *png_set_read_fn_allow_revoke(png_structrp png, png_voidp io_ptr,
				   png_rw_ptr read_data_fn) {
    // Place the original arguments onto the heap:
    png_read_fn_data_wrapped_t *wrapped = malloc(sizeof(png_read_fn_data_wrapped_t));
    wrapped->io_ptr = io_ptr;
    wrapped->read_data_fn = read_data_fn;

    // Register our own callback:
    png_set_read_fn(png, wrapped, png_read_fn_allow_revoke_wrapper);

    // Return a reference to the wrapped data, so it can be freed later:
    return wrapped;
}

void png_free_read_fn_allow_revoke_state(void *state) {
    free(state);
}

// -----------------------------------------------------------------------------

void decode_png_read_cb(png_structrp png_ptr, uint8_t *buf_ptr,
                        png_size_t count) {
  const uint8_t **image_ptr = png_get_io_ptr(png_ptr);
  /* printf("Reading from image ptr: %p, %d\n", *image_ptr, count); */
  memcpy(buf_ptr, *image_ptr, count);
  *image_ptr += count;
}

uint8_t **decode_png(png_structrp png_ptr, png_inforp info_ptr,
                     const uint8_t *png_image, png_uint_32 *rows,
                     png_uint_32 *cols) {
  if (0 != setjmp(png_jmpbuf(png_ptr))) {
    return false;
  }

  const uint8_t *image_ptr = png_image;

  png_set_read_fn(png_ptr, &image_ptr, decode_png_read_cb);

  png_read_info(png_ptr, info_ptr);

  *rows = png_get_image_height(png_ptr, info_ptr);
  *cols = png_get_rowbytes(png_ptr, info_ptr);

  uint8_t **row_ptrs = calloc(sizeof(void *), *rows);
  if (row_ptrs == NULL) {
    return false;
  }

  for (size_t i = 0; i < *rows; i++) {
    row_ptrs[i] = malloc(*cols);
    if (row_ptrs[i] == NULL) {
      return false;
    }
  }

  png_read_image(png_ptr, row_ptrs);

  return row_ptrs;
}
