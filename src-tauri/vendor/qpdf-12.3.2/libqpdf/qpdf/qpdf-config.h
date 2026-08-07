/* PicTrim static build configuration for QPDF 12.3.2. */
#define DEFAULT_CRYPTO "native"
#define USE_CRYPTO_NATIVE 1
#define HAVE_INTTYPES_H 1
#define HAVE_STDINT_H 1
#define SIZEOF_SIZE_T 8

#if !defined(_WIN32)
# define _FILE_OFFSET_BITS 64
# define HAVE_FSEEKO 1
# define HAVE_LOCALTIME_R 1
#endif

#if defined(__APPLE__) || defined(__linux__)
# define HAVE_TM_GMTOFF 1
#endif
