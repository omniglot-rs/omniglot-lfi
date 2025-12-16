#include <png.h>
#include <setjmp.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

bool png_read_info_nojmp(png_structrp png_ptr, png_inforp info_ptr);
bool png_read_image_nojmp(png_structrp png_ptr, png_bytepp image);

void *png_set_read_fn_allow_revoke(png_structrp png, png_voidp io_ptr,
				   png_rw_ptr read_data_fn);
void png_free_read_fn_allow_revoke_state(void *state);

// -----------------------------------------------------------------------------

uint8_t **decode_png(png_structrp png_ptr, png_inforp info_ptr,
                     const uint8_t *png_image, png_uint_32 *rows,
                     png_uint_32 *cols);
