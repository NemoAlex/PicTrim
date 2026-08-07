use std::ffi::{c_char, c_int, c_uchar, c_void};

pub(crate) type DocumentHandle = c_void;

#[repr(C)]
#[derive(Debug, Default)]
pub(crate) struct Buffer {
    pub data: *mut c_uchar,
    pub len: usize,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ImageInfo {
    pub object_id: i32,
    pub generation: i32,
    pub first_page: u32,
    pub image_index: u32,
    pub width: u32,
    pub height: u32,
    pub bits_per_component: u32,
    pub components: u32,
    pub filter: i32,
    pub color_space: i32,
    pub indexed_base_color_space: i32,
    pub indexed_high_value: u32,
    pub smask_object_id: i32,
    pub smask_generation: i32,
    pub mask_object_id: i32,
    pub mask_generation: i32,
    pub has_color_key_mask: u8,
    pub image_mask: u8,
    pub decode_mode: u8,
}

unsafe extern "C" {
    pub fn pt_qpdf_open(
        path: *const c_char,
        out_document: *mut *mut DocumentHandle,
        out_error: *mut *mut c_char,
    ) -> c_int;
    pub fn pt_qpdf_close(document: *mut DocumentHandle);
    pub fn pt_qpdf_is_encrypted(document: *mut DocumentHandle) -> c_int;
    pub fn pt_qpdf_has_signatures(document: *mut DocumentHandle) -> c_int;
    pub fn pt_qpdf_image_count(document: *mut DocumentHandle) -> usize;
    pub fn pt_qpdf_image_info_at(
        document: *mut DocumentHandle,
        index: usize,
        out_info: *mut ImageInfo,
    ) -> c_int;
    pub fn pt_qpdf_object_info(
        document: *mut DocumentHandle,
        object_id: i32,
        generation: i32,
        out_info: *mut ImageInfo,
    ) -> c_int;
    pub fn pt_qpdf_read_raw(
        document: *mut DocumentHandle,
        object_id: i32,
        generation: i32,
        out_buffer: *mut Buffer,
    ) -> c_int;
    pub fn pt_qpdf_read_decoded(
        document: *mut DocumentHandle,
        object_id: i32,
        generation: i32,
        out_buffer: *mut Buffer,
    ) -> c_int;
    pub fn pt_qpdf_read_palette(
        document: *mut DocumentHandle,
        object_id: i32,
        generation: i32,
        out_buffer: *mut Buffer,
    ) -> c_int;
    pub fn pt_qpdf_read_icc_profile(
        document: *mut DocumentHandle,
        object_id: i32,
        generation: i32,
        out_buffer: *mut Buffer,
    ) -> c_int;
    pub fn pt_qpdf_replace_image(
        document: *mut DocumentHandle,
        object_id: i32,
        generation: i32,
        data: *const c_uchar,
        len: usize,
        width: u32,
        height: u32,
        components: u32,
        color_space: i32,
        filter: i32,
    ) -> c_int;
    pub fn pt_qpdf_save(document: *mut DocumentHandle, path: *const c_char) -> c_int;
    pub fn pt_qpdf_check_file(path: *const c_char, out_error: *mut *mut c_char) -> c_int;
    pub fn pt_qpdf_take_error(document: *mut DocumentHandle) -> *mut c_char;
    pub fn pt_qpdf_free_error(error: *mut c_char);
    pub fn pt_qpdf_free_buffer(buffer: Buffer);
}
