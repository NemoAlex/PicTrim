mod ffi;

use std::ffi::{CStr, CString};
use std::marker::PhantomData;
use std::path::Path;
use std::ptr;
use std::rc::Rc;

pub(crate) const FILTER_DCT: i32 = 1;
pub(crate) const FILTER_JPX: i32 = 2;
pub(crate) const FILTER_FLATE: i32 = 3;
pub(crate) const FILTER_LZW: i32 = 4;

pub(crate) const COLOR_GRAY: i32 = 1;
pub(crate) const COLOR_RGB: i32 = 2;
pub(crate) const COLOR_CMYK: i32 = 3;
pub(crate) const COLOR_INDEXED: i32 = 4;
pub(crate) const COLOR_ICC: i32 = 5;

pub(crate) type ImageInfo = ffi::ImageInfo;

pub(crate) struct Replacement<'a> {
    pub data: &'a [u8],
    pub width: u32,
    pub height: u32,
    pub components: u32,
    pub color_space: i32,
    pub filter: i32,
}

pub(crate) struct Document {
    handle: *mut ffi::DocumentHandle,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl Document {
    pub(crate) fn open(path: &Path) -> Result<Self, String> {
        let path = path_c_string(path)?;
        let mut handle = ptr::null_mut();
        let mut error = ptr::null_mut();
        let ok = unsafe { ffi::pt_qpdf_open(path.as_ptr(), &mut handle, &mut error) };
        if ok == 0 || handle.is_null() {
            return Err(unsafe { take_owned_error(error, "无法打开 PDF") });
        }
        Ok(Self {
            handle,
            _not_send_or_sync: PhantomData,
        })
    }

    pub(crate) fn is_encrypted(&self) -> bool {
        unsafe { ffi::pt_qpdf_is_encrypted(self.handle) != 0 }
    }

    pub(crate) fn has_signatures(&self) -> bool {
        unsafe { ffi::pt_qpdf_has_signatures(self.handle) != 0 }
    }

    pub(crate) fn images(&mut self) -> Result<Vec<ImageInfo>, String> {
        let count = unsafe { ffi::pt_qpdf_image_count(self.handle) };
        let mut result = Vec::with_capacity(count);
        for index in 0..count {
            let mut info = ImageInfo::default();
            self.call(|handle| unsafe { ffi::pt_qpdf_image_info_at(handle, index, &mut info) })?;
            result.push(info);
        }
        Ok(result)
    }

    pub(crate) fn object_info(
        &mut self,
        object_id: i32,
        generation: i32,
    ) -> Result<ImageInfo, String> {
        let mut info = ImageInfo::default();
        self.call(|handle| unsafe {
            ffi::pt_qpdf_object_info(handle, object_id, generation, &mut info)
        })?;
        Ok(info)
    }

    pub(crate) fn read_raw(&mut self, info: &ImageInfo) -> Result<Vec<u8>, String> {
        self.read_buffer(info, ffi::pt_qpdf_read_raw)
    }

    pub(crate) fn read_decoded(&mut self, info: &ImageInfo) -> Result<Vec<u8>, String> {
        self.read_buffer(info, ffi::pt_qpdf_read_decoded)
    }

    pub(crate) fn read_palette(&mut self, info: &ImageInfo) -> Result<Vec<u8>, String> {
        self.read_buffer(info, ffi::pt_qpdf_read_palette)
    }

    #[allow(dead_code)]
    pub(crate) fn read_icc_profile(&mut self, info: &ImageInfo) -> Result<Vec<u8>, String> {
        self.read_buffer(info, ffi::pt_qpdf_read_icc_profile)
    }

    pub(crate) fn replace_image(
        &mut self,
        info: &ImageInfo,
        replacement: Replacement<'_>,
    ) -> Result<(), String> {
        self.call(|handle| unsafe {
            ffi::pt_qpdf_replace_image(
                handle,
                info.object_id,
                info.generation,
                replacement.data.as_ptr(),
                replacement.data.len(),
                replacement.width,
                replacement.height,
                replacement.components,
                replacement.color_space,
                replacement.filter,
            )
        })
    }

    pub(crate) fn save(&mut self, path: &Path) -> Result<(), String> {
        let path = path_c_string(path)?;
        self.call(|handle| unsafe { ffi::pt_qpdf_save(handle, path.as_ptr()) })
    }

    fn read_buffer(
        &mut self,
        info: &ImageInfo,
        read: unsafe extern "C" fn(*mut ffi::DocumentHandle, i32, i32, *mut ffi::Buffer) -> i32,
    ) -> Result<Vec<u8>, String> {
        let mut buffer = ffi::Buffer::default();
        self.call(|handle| unsafe { read(handle, info.object_id, info.generation, &mut buffer) })?;
        let result = if buffer.data.is_null() || buffer.len == 0 {
            Vec::new()
        } else {
            unsafe { std::slice::from_raw_parts(buffer.data, buffer.len).to_vec() }
        };
        unsafe { ffi::pt_qpdf_free_buffer(buffer) };
        Ok(result)
    }

    fn call(
        &mut self,
        operation: impl FnOnce(*mut ffi::DocumentHandle) -> i32,
    ) -> Result<(), String> {
        if operation(self.handle) != 0 {
            return Ok(());
        }
        let error = unsafe { ffi::pt_qpdf_take_error(self.handle) };
        Err(unsafe { take_owned_error(error, "QPDF 操作失败") })
    }
}

impl Drop for Document {
    fn drop(&mut self) {
        unsafe { ffi::pt_qpdf_close(self.handle) };
    }
}

pub(crate) fn check(path: &Path) -> Result<(), String> {
    let path = path_c_string(path)?;
    let mut error = ptr::null_mut();
    if unsafe { ffi::pt_qpdf_check_file(path.as_ptr(), &mut error) } != 0 {
        Ok(())
    } else {
        Err(unsafe { take_owned_error(error, "PDF 结构校验失败") })
    }
}

fn path_c_string(path: &Path) -> Result<CString, String> {
    CString::new(path.to_string_lossy().as_bytes()).map_err(|_| "PDF 路径包含空字符".to_string())
}

unsafe fn take_owned_error(error: *mut std::ffi::c_char, fallback: &str) -> String {
    if error.is_null() {
        return fallback.to_string();
    }
    let message = unsafe { CStr::from_ptr(error) }
        .to_string_lossy()
        .into_owned();
    unsafe { ffi::pt_qpdf_free_error(error) };
    if message.is_empty() {
        fallback.to_string()
    } else {
        message
    }
}
