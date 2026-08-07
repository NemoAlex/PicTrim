#include "pictrim_qpdf.h"

#include <qpdf/Buffer.hh>
#include <qpdf/Pl_Discard.hh>
#include <qpdf/QPDF.hh>
#include <qpdf/QPDFObjectHandle.hh>
#include <qpdf/QPDFPageDocumentHelper.hh>
#include <qpdf/QPDFPageObjectHelper.hh>
#include <qpdf/QPDFWriter.hh>

#include <cstdlib>
#include <cstring>
#include <map>
#include <memory>
#include <set>
#include <stdexcept>
#include <string>
#include <utility>
#include <vector>

struct ImageRecord
{
    QPDFObjectHandle object;
    uint32_t first_page;
    uint32_t image_index;
};

struct pt_qpdf_document
{
    std::unique_ptr<QPDF> pdf;
    std::vector<ImageRecord> images;
    std::string last_error;
    bool encrypted{false};
    bool signatures{false};
};

namespace
{
char*
copy_error(std::string const& message)
{
    auto* result = static_cast<char*>(std::malloc(message.size() + 1));
    if (result != nullptr) {
        std::memcpy(result, message.c_str(), message.size() + 1);
    }
    return result;
}

template <typename Fn>
int
guard(pt_qpdf_document* document, Fn&& fn)
{
    if (document == nullptr) {
        return 0;
    }
    try {
        document->last_error.clear();
        fn();
        return 1;
    } catch (std::exception const& e) {
        document->last_error = e.what();
        return 0;
    } catch (...) {
        document->last_error = "unknown QPDF error";
        return 0;
    }
}

QPDFObjectHandle
object_by_id(pt_qpdf_document* document, int32_t object_id, int32_t generation)
{
    if (object_id <= 0 || generation < 0) {
        throw std::runtime_error("invalid PDF object identifier");
    }
    auto object = document->pdf->getObjectByID(object_id, generation);
    if (!object.isStream()) {
        throw std::runtime_error("PDF object is not an image stream");
    }
    return object;
}

int32_t
filter_kind(QPDFObjectHandle dict)
{
    auto filter = dict.getKey("/Filter");
    if (filter.isArray()) {
        if (filter.getArrayNItems() != 1) {
            return PT_QPDF_FILTER_UNKNOWN;
        }
        filter = filter.getArrayItem(0);
    }
    if (filter.isNull()) {
        return PT_QPDF_FILTER_FLATE;
    }
    if (filter.isNameAndEquals("/DCTDecode") || filter.isNameAndEquals("/DCT")) {
        return PT_QPDF_FILTER_DCT;
    }
    if (filter.isNameAndEquals("/JPXDecode")) {
        return PT_QPDF_FILTER_JPX;
    }
    if (filter.isNameAndEquals("/FlateDecode") || filter.isNameAndEquals("/Fl")) {
        return PT_QPDF_FILTER_FLATE;
    }
    if (filter.isNameAndEquals("/LZWDecode") || filter.isNameAndEquals("/LZW")) {
        return PT_QPDF_FILTER_LZW;
    }
    return PT_QPDF_FILTER_UNKNOWN;
}

int32_t
simple_color_space(QPDFObjectHandle color_space)
{
    if (color_space.isNameAndEquals("/DeviceGray") || color_space.isNameAndEquals("/G")) {
        return PT_QPDF_COLOR_GRAY;
    }
    if (color_space.isNameAndEquals("/DeviceRGB") || color_space.isNameAndEquals("/RGB")) {
        return PT_QPDF_COLOR_RGB;
    }
    if (color_space.isNameAndEquals("/DeviceCMYK") || color_space.isNameAndEquals("/CMYK")) {
        return PT_QPDF_COLOR_CMYK;
    }
    return PT_QPDF_COLOR_UNKNOWN;
}

uint32_t
components_for_color(int32_t color)
{
    switch (color) {
    case PT_QPDF_COLOR_GRAY:
        return 1;
    case PT_QPDF_COLOR_RGB:
        return 3;
    case PT_QPDF_COLOR_CMYK:
        return 4;
    default:
        return 0;
    }
}

QPDFObjectHandle
resolve_color_space(QPDFObjectHandle object)
{
    auto color_space = object.getDict().getKey("/ColorSpace");
    if (color_space.isName() || color_space.isArray()) {
        return color_space;
    }
    return QPDFObjectHandle::newNull();
}

void
fill_reference(QPDFObjectHandle value, int32_t& object_id, int32_t& generation)
{
    object_id = 0;
    generation = 0;
    if (value.isStream()) {
        object_id = value.getObjectID();
        generation = value.getGeneration();
    }
}

pt_qpdf_image_info
make_info(QPDFObjectHandle object, uint32_t page, uint32_t index)
{
    pt_qpdf_image_info info{};
    auto dict = object.getDict();
    info.object_id = object.getObjectID();
    info.generation = object.getGeneration();
    info.first_page = page;
    info.image_index = index;
    info.width = static_cast<uint32_t>(dict.getKey("/Width").getUIntValue());
    info.height = static_cast<uint32_t>(dict.getKey("/Height").getUIntValue());
    auto bits = dict.getKey("/BitsPerComponent");
    info.bits_per_component = bits.isInteger() ? static_cast<uint32_t>(bits.getUIntValue()) : 1;
    info.filter = filter_kind(dict);
    info.image_mask = dict.getKey("/ImageMask").isBool() && dict.getKey("/ImageMask").getBoolValue();

    auto color_space = resolve_color_space(object);
    info.color_space = simple_color_space(color_space);
    if (color_space.isArray() && color_space.getArrayNItems() >= 1) {
        auto family = color_space.getArrayItem(0);
        if (family.isNameAndEquals("/Indexed") || family.isNameAndEquals("/I")) {
            info.color_space = PT_QPDF_COLOR_INDEXED;
            if (color_space.getArrayNItems() >= 3) {
                info.indexed_base_color_space = simple_color_space(color_space.getArrayItem(1));
                auto high = color_space.getArrayItem(2);
                if (high.isInteger()) {
                    info.indexed_high_value = static_cast<uint32_t>(high.getUIntValue());
                }
            }
        } else if (family.isNameAndEquals("/ICCBased") && color_space.getArrayNItems() >= 2) {
            info.color_space = PT_QPDF_COLOR_ICC;
            auto profile = color_space.getArrayItem(1);
            if (profile.isStream()) {
                auto n = profile.getDict().getKey("/N");
                if (n.isInteger()) {
                    info.components = static_cast<uint32_t>(n.getUIntValue());
                }
            }
        }
    }
    if (info.components == 0) {
        info.components = components_for_color(info.color_space);
    }
    if (info.color_space == PT_QPDF_COLOR_INDEXED) {
        info.components = 1;
    }

    auto decode = dict.getKey("/Decode");
    if (decode.isArray()) {
        auto decode_components = info.color_space == PT_QPDF_COLOR_INDEXED ? 1U : info.components;
        if (decode_components == 0 ||
            decode.getArrayNItems() != static_cast<int>(decode_components * 2)) {
            info.decode_mode = 2;
        } else {
            bool normal = true;
            bool inverted = true;
            double normal_high = info.color_space == PT_QPDF_COLOR_INDEXED
                ? static_cast<double>(info.indexed_high_value)
                : 1.0;
            for (uint32_t i = 0; i < decode_components; ++i) {
                auto low = decode.getArrayItem(static_cast<int>(i * 2));
                auto high = decode.getArrayItem(static_cast<int>(i * 2 + 1));
                if (!low.isNumber() || !high.isNumber()) {
                    normal = false;
                    inverted = false;
                    break;
                }
                auto low_value = low.getNumericValue();
                auto high_value = high.getNumericValue();
                normal = normal && low_value == 0.0 && high_value == normal_high;
                inverted = inverted && low_value == normal_high && high_value == 0.0;
            }
            info.decode_mode = normal ? 0 : (inverted ? 1 : 2);
        }
    }

    fill_reference(dict.getKey("/SMask"), info.smask_object_id, info.smask_generation);
    auto mask = dict.getKey("/Mask");
    fill_reference(mask, info.mask_object_id, info.mask_generation);
    info.has_color_key_mask = mask.isArray();
    return info;
}

bool
has_signature_field(QPDFObjectHandle field, std::set<QPDFObjGen>& visited)
{
    auto og = field.getObjGen();
    if (og.getObj() != 0 && !visited.insert(og).second) {
        return false;
    }
    if (field.isDictionary()) {
        if (field.getKey("/FT").isNameAndEquals("/Sig")) {
            return true;
        }
        auto value = field.getKey("/V");
        if (value.isDictionaryOfType("/Sig", "") || value.getKey("/Type").isNameAndEquals("/Sig")) {
            return true;
        }
        auto kids = field.getKey("/Kids");
        if (kids.isArray()) {
            for (int i = 0; i < kids.getArrayNItems(); ++i) {
                if (has_signature_field(kids.getArrayItem(i), visited)) {
                    return true;
                }
            }
        }
    }
    return false;
}

bool
detect_signatures(QPDF& pdf)
{
    auto acro_form = pdf.getRoot().getKey("/AcroForm");
    if (!acro_form.isDictionary()) {
        return false;
    }
    auto fields = acro_form.getKey("/Fields");
    if (!fields.isArray()) {
        return false;
    }
    std::set<QPDFObjGen> visited;
    for (int i = 0; i < fields.getArrayNItems(); ++i) {
        if (has_signature_field(fields.getArrayItem(i), visited)) {
            return true;
        }
    }
    return false;
}

void
enumerate_images(pt_qpdf_document& document)
{
    std::set<QPDFObjGen> seen;
    uint32_t page_number = 0;
    uint32_t image_index = 0;
    for (auto& page: QPDFPageDocumentHelper(*document.pdf).getAllPages()) {
        ++page_number;
        page.externalizeInlineImages(0, false);
        page.forEachImage(
            true,
            [&](QPDFObjectHandle& object, QPDFObjectHandle&, std::string const&) {
                auto og = object.getObjGen();
                if (seen.insert(og).second) {
                    ++image_index;
                    document.images.push_back(ImageRecord{object, page_number, image_index});
                }
            });
    }
}

void
copy_buffer(std::shared_ptr<Buffer> const& source, pt_qpdf_buffer* output)
{
    if (output == nullptr) {
        throw std::runtime_error("missing output buffer");
    }
    output->data = nullptr;
    output->len = 0;
    if (source->getSize() == 0) {
        return;
    }
    auto* data = static_cast<uint8_t*>(std::malloc(source->getSize()));
    if (data == nullptr) {
        throw std::bad_alloc();
    }
    std::memcpy(data, source->getBuffer(), source->getSize());
    output->data = data;
    output->len = source->getSize();
}

QPDFObjectHandle
color_space_object(int32_t color_space)
{
    switch (color_space) {
    case PT_QPDF_COLOR_GRAY:
        return QPDFObjectHandle::newName("/DeviceGray");
    case PT_QPDF_COLOR_RGB:
        return QPDFObjectHandle::newName("/DeviceRGB");
    case PT_QPDF_COLOR_CMYK:
        return QPDFObjectHandle::newName("/DeviceCMYK");
    default:
        throw std::runtime_error("unsupported output PDF color space");
    }
}
}

extern "C" int
pt_qpdf_open(const char* path, pt_qpdf_document** out_document, char** out_error)
{
    if (out_document == nullptr || path == nullptr) {
        if (out_error != nullptr) {
            *out_error = copy_error("invalid QPDF open arguments");
        }
        return 0;
    }
    *out_document = nullptr;
    if (out_error != nullptr) {
        *out_error = nullptr;
    }
    try {
        auto document = std::make_unique<pt_qpdf_document>();
        document->pdf = std::make_unique<QPDF>();
        document->pdf->processFile(path, "");
        document->encrypted = document->pdf->isEncrypted();
        document->signatures = detect_signatures(*document->pdf);
        enumerate_images(*document);
        *out_document = document.release();
        return 1;
    } catch (std::exception const& e) {
        if (out_error != nullptr) {
            *out_error = copy_error(e.what());
        }
        return 0;
    } catch (...) {
        if (out_error != nullptr) {
            *out_error = copy_error("unknown QPDF open error");
        }
        return 0;
    }
}

extern "C" void
pt_qpdf_close(pt_qpdf_document* document)
{
    delete document;
}

extern "C" int
pt_qpdf_is_encrypted(pt_qpdf_document* document)
{
    return document != nullptr && document->encrypted;
}

extern "C" int
pt_qpdf_has_signatures(pt_qpdf_document* document)
{
    return document != nullptr && document->signatures;
}

extern "C" size_t
pt_qpdf_image_count(pt_qpdf_document* document)
{
    return document == nullptr ? 0 : document->images.size();
}

extern "C" int
pt_qpdf_image_info_at(pt_qpdf_document* document, size_t index, pt_qpdf_image_info* out_info)
{
    return guard(document, [&] {
        if (out_info == nullptr || index >= document->images.size()) {
            throw std::runtime_error("PDF image index is out of range");
        }
        auto const& record = document->images.at(index);
        *out_info = make_info(record.object, record.first_page, record.image_index);
    });
}

extern "C" int
pt_qpdf_object_info(
    pt_qpdf_document* document,
    int32_t object_id,
    int32_t generation,
    pt_qpdf_image_info* out_info)
{
    return guard(document, [&] {
        if (out_info == nullptr) {
            throw std::runtime_error("missing PDF image info output");
        }
        *out_info = make_info(object_by_id(document, object_id, generation), 0, 0);
    });
}

extern "C" int
pt_qpdf_read_raw(
    pt_qpdf_document* document,
    int32_t object_id,
    int32_t generation,
    pt_qpdf_buffer* out_buffer)
{
    return guard(document, [&] {
        copy_buffer(object_by_id(document, object_id, generation).getRawStreamData(), out_buffer);
    });
}

extern "C" int
pt_qpdf_read_decoded(
    pt_qpdf_document* document,
    int32_t object_id,
    int32_t generation,
    pt_qpdf_buffer* out_buffer)
{
    return guard(document, [&] {
        copy_buffer(
            object_by_id(document, object_id, generation).getStreamData(qpdf_dl_all), out_buffer);
    });
}

extern "C" int
pt_qpdf_read_palette(
    pt_qpdf_document* document,
    int32_t object_id,
    int32_t generation,
    pt_qpdf_buffer* out_buffer)
{
    return guard(document, [&] {
        auto color_space = resolve_color_space(object_by_id(document, object_id, generation));
        if (!color_space.isArray() || color_space.getArrayNItems() < 4) {
            throw std::runtime_error("indexed image has no lookup table");
        }
        auto lookup = color_space.getArrayItem(3);
        if (lookup.isString()) {
            auto value = lookup.getStringValue();
            auto buffer = std::make_shared<Buffer>(value.size());
            if (!value.empty()) {
                std::memcpy(buffer->getBuffer(), value.data(), value.size());
            }
            copy_buffer(buffer, out_buffer);
        } else if (lookup.isStream()) {
            copy_buffer(lookup.getStreamData(qpdf_dl_all), out_buffer);
        } else {
            throw std::runtime_error("unsupported indexed image lookup table");
        }
    });
}

extern "C" int
pt_qpdf_read_icc_profile(
    pt_qpdf_document* document,
    int32_t object_id,
    int32_t generation,
    pt_qpdf_buffer* out_buffer)
{
    return guard(document, [&] {
        auto color_space = resolve_color_space(object_by_id(document, object_id, generation));
        if (!color_space.isArray() || color_space.getArrayNItems() < 2 ||
            !color_space.getArrayItem(0).isNameAndEquals("/ICCBased") ||
            !color_space.getArrayItem(1).isStream()) {
            throw std::runtime_error("image has no ICC profile");
        }
        copy_buffer(color_space.getArrayItem(1).getStreamData(qpdf_dl_all), out_buffer);
    });
}

extern "C" int
pt_qpdf_replace_image(
    pt_qpdf_document* document,
    int32_t object_id,
    int32_t generation,
    const uint8_t* data,
    size_t len,
    uint32_t width,
    uint32_t height,
    uint32_t components,
    int32_t color_space,
    int32_t filter)
{
    return guard(document, [&] {
        if (data == nullptr || len == 0 || width == 0 || height == 0) {
            throw std::runtime_error("invalid replacement image data");
        }
        auto object = object_by_id(document, object_id, generation);
        auto dict = object.getDict();
        auto hard_mask = dict.getKey("/Mask");
        if (hard_mask.isStream()) {
            dict.replaceKey("/SMask", hard_mask);
            dict.removeKey("/Mask");
        }
        dict.replaceKey("/Type", QPDFObjectHandle::newName("/XObject"));
        dict.replaceKey("/Subtype", QPDFObjectHandle::newName("/Image"));
        dict.replaceKey("/Width", QPDFObjectHandle::newInteger(width));
        dict.replaceKey("/Height", QPDFObjectHandle::newInteger(height));
        dict.replaceKey("/BitsPerComponent", QPDFObjectHandle::newInteger(8));
        dict.replaceKey("/ColorSpace", color_space_object(color_space));
        dict.removeKey("/DecodeParms");
        dict.removeKey("/Decode");
        dict.removeKey("/ImageMask");
        if (components != components_for_color(color_space)) {
            throw std::runtime_error("replacement component count does not match color space");
        }
        std::string bytes(reinterpret_cast<char const*>(data), len);
        if (filter == PT_QPDF_FILTER_DCT) {
            object.replaceStreamData(
                bytes, QPDFObjectHandle::newName("/DCTDecode"), QPDFObjectHandle::newNull());
            object.setFilterOnWrite(false);
        } else if (filter == PT_QPDF_FILTER_FLATE) {
            object.replaceStreamData(bytes, QPDFObjectHandle::newNull(), QPDFObjectHandle::newNull());
            object.setFilterOnWrite(true);
        } else {
            throw std::runtime_error("unsupported replacement image filter");
        }
    });
}

extern "C" int
pt_qpdf_save(pt_qpdf_document* document, const char* path)
{
    return guard(document, [&] {
        if (path == nullptr) {
            throw std::runtime_error("missing PDF output path");
        }
        QPDFWriter writer(*document->pdf, path);
        writer.setPreserveEncryption(true);
        writer.setObjectStreamMode(qpdf_o_generate);
        writer.setCompressStreams(true);
        writer.write();
    });
}

extern "C" int
pt_qpdf_check_file(const char* path, char** out_error)
{
    if (out_error != nullptr) {
        *out_error = nullptr;
    }
    try {
        QPDF pdf;
        pdf.processFile(path, "");
        QPDFWriter writer(pdf);
        Pl_Discard discard;
        writer.setOutputPipeline(&discard);
        writer.setDecodeLevel(qpdf_dl_all);
        writer.setCompressStreams(false);
        writer.write();
        for (auto& page: QPDFPageDocumentHelper(pdf).getAllPages()) {
            page.parseContents(nullptr);
        }
        if (!pdf.getWarnings().empty()) {
            throw std::runtime_error("QPDF reported warnings while validating output");
        }
        return 1;
    } catch (std::exception const& e) {
        if (out_error != nullptr) {
            *out_error = copy_error(e.what());
        }
        return 0;
    } catch (...) {
        if (out_error != nullptr) {
            *out_error = copy_error("unknown QPDF validation error");
        }
        return 0;
    }
}

extern "C" char*
pt_qpdf_take_error(pt_qpdf_document* document)
{
    if (document == nullptr || document->last_error.empty()) {
        return copy_error("unknown QPDF error");
    }
    auto* error = copy_error(document->last_error);
    document->last_error.clear();
    return error;
}

extern "C" void
pt_qpdf_free_error(char* error)
{
    std::free(error);
}

extern "C" void
pt_qpdf_free_buffer(pt_qpdf_buffer buffer)
{
    std::free(buffer.data);
}
