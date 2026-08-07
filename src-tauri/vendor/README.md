# Vendored native PDF dependencies

PicTrim builds these sources statically into the application:

- QPDF 12.3.2 (`qpdf-12.3.2`), Apache License 2.0
- zlib 1.3.1 (`zlib-1.3.1`), zlib License
- Independent JPEG Group libjpeg 9f (`jpeg-9f`), IJG License

The source archives are pinned so release builds do not require QPDF, CMake,
libclang, zlib, or libjpeg to be installed on the build machine. See each
directory for its upstream license and notice files.
