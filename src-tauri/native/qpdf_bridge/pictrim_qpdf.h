#ifndef PICTRIM_QPDF_H
#define PICTRIM_QPDF_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct pt_qpdf_document pt_qpdf_document;

typedef struct pt_qpdf_buffer {
    uint8_t* data;
    size_t len;
} pt_qpdf_buffer;

enum pt_qpdf_filter {
    PT_QPDF_FILTER_UNKNOWN = 0,
    PT_QPDF_FILTER_DCT = 1,
    PT_QPDF_FILTER_JPX = 2,
    PT_QPDF_FILTER_FLATE = 3,
    PT_QPDF_FILTER_LZW = 4
};

enum pt_qpdf_color_space {
    PT_QPDF_COLOR_UNKNOWN = 0,
    PT_QPDF_COLOR_GRAY = 1,
    PT_QPDF_COLOR_RGB = 2,
    PT_QPDF_COLOR_CMYK = 3,
    PT_QPDF_COLOR_INDEXED = 4,
    PT_QPDF_COLOR_ICC = 5
};

typedef struct pt_qpdf_image_info {
    int32_t object_id;
    int32_t generation;
    uint32_t first_page;
    uint32_t image_index;
    uint32_t width;
    uint32_t height;
    uint32_t bits_per_component;
    uint32_t components;
    int32_t filter;
    int32_t color_space;
    int32_t indexed_base_color_space;
    uint32_t indexed_high_value;
    int32_t smask_object_id;
    int32_t smask_generation;
    int32_t mask_object_id;
    int32_t mask_generation;
    uint8_t has_color_key_mask;
    uint8_t image_mask;
    uint8_t decode_mode;
} pt_qpdf_image_info;

int pt_qpdf_open(const char* path, pt_qpdf_document** out_document, char** out_error);
void pt_qpdf_close(pt_qpdf_document* document);
int pt_qpdf_is_encrypted(pt_qpdf_document* document);
int pt_qpdf_has_signatures(pt_qpdf_document* document);
size_t pt_qpdf_image_count(pt_qpdf_document* document);
int pt_qpdf_image_info_at(
    pt_qpdf_document* document,
    size_t index,
    pt_qpdf_image_info* out_info);
int pt_qpdf_object_info(
    pt_qpdf_document* document,
    int32_t object_id,
    int32_t generation,
    pt_qpdf_image_info* out_info);
int pt_qpdf_read_raw(
    pt_qpdf_document* document,
    int32_t object_id,
    int32_t generation,
    pt_qpdf_buffer* out_buffer);
int pt_qpdf_read_decoded(
    pt_qpdf_document* document,
    int32_t object_id,
    int32_t generation,
    pt_qpdf_buffer* out_buffer);
int pt_qpdf_read_palette(
    pt_qpdf_document* document,
    int32_t object_id,
    int32_t generation,
    pt_qpdf_buffer* out_buffer);
int pt_qpdf_read_icc_profile(
    pt_qpdf_document* document,
    int32_t object_id,
    int32_t generation,
    pt_qpdf_buffer* out_buffer);
int pt_qpdf_replace_image(
    pt_qpdf_document* document,
    int32_t object_id,
    int32_t generation,
    const uint8_t* data,
    size_t len,
    uint32_t width,
    uint32_t height,
    uint32_t components,
    int32_t color_space,
    int32_t filter);
int pt_qpdf_save(pt_qpdf_document* document, const char* path);
int pt_qpdf_check_file(const char* path, char** out_error);
char* pt_qpdf_take_error(pt_qpdf_document* document);
void pt_qpdf_free_error(char* error);
void pt_qpdf_free_buffer(pt_qpdf_buffer buffer);

#ifdef __cplusplus
}
#endif

#endif
