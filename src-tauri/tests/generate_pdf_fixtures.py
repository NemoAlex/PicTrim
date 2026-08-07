#!/usr/bin/env python3
"""Generate the small PDFs used by PicTrim's PDF smoke tests."""

from __future__ import annotations

import io
import zlib
from pathlib import Path

from PIL import Image, ImageCms
from pypdf import PdfReader, PdfWriter


ROOT = Path(__file__).resolve().parent / "fixtures"


def stream(dictionary: bytes, data: bytes) -> bytes:
    return b"<< " + dictionary + f" /Length {len(data)} >>\nstream\n".encode() + data + b"\nendstream"


def write_pdf(name: str, objects: list[bytes]) -> Path:
    data = bytearray(b"%PDF-1.7\n%\xd0\xd4\xc5\xd8\n")
    offsets: list[int] = []
    for index, obj in enumerate(objects, 1):
        offsets.append(len(data))
        data.extend(f"{index} 0 obj\n".encode())
        data.extend(obj)
        data.extend(b"\nendobj\n")
    xref = len(data)
    data.extend(f"xref\n0 {len(objects) + 1}\n".encode())
    data.extend(b"0000000000 65535 f \n")
    for offset in offsets:
        data.extend(f"{offset:010} 00000 n \n".encode())
    data.extend(
        f"trailer\n<< /Size {len(objects) + 1} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n".encode()
    )
    path = ROOT / name
    path.write_bytes(data)
    return path


def base_objects(image: bytes, content: bytes | None = None, catalog: bytes | None = None) -> list[bytes]:
    content = content or b"BT /F1 12 Tf 20 90 Td (PicTrim PDF text) Tj ET q 40 0 0 40 20 20 cm /Im1 Do Q"
    return [
        catalog or b"<< /Type /Catalog /Pages 2 0 R >>",
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 140 120] /Resources << /XObject << /Im1 5 0 R >> /Font << /F1 6 0 R >> >> /Contents 4 0 R >>",
        stream(b"", content),
        image,
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
    ]


def flate_image(width: int, height: int, color_space: bytes, pixels: bytes, extra: bytes = b"") -> bytes:
    dictionary = (
        f"/Type /XObject /Subtype /Image /Width {width} /Height {height} ".encode()
        + b"/ColorSpace "
        + color_space
        + b" /BitsPerComponent 8 /Filter /FlateDecode "
        + extra
    )
    return stream(dictionary, zlib.compress(pixels))


def jpeg_image() -> bytes:
    image = Image.new("RGB", (2, 2))
    image.putdata([(255, 0, 0), (0, 255, 0), (0, 0, 255), (255, 255, 0)])
    output = io.BytesIO()
    image.save(output, "JPEG", quality=90)
    return stream(
        b"/Type /XObject /Subtype /Image /Width 2 /Height 2 /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /DCTDecode",
        output.getvalue(),
    )


def encrypt(source: Path, name: str, user_password: str) -> None:
    reader = PdfReader(source)
    writer = PdfWriter()
    writer.append_pages_from_reader(reader)
    writer.encrypt(user_password=user_password, owner_password="pictrim-owner")
    with (ROOT / name).open("wb") as output:
        writer.write(output)


def srgb_profile() -> bytes:
    path = ROOT / "srgb.icc"
    if path.exists():
        return path.read_bytes()
    profile = ImageCms.ImageCmsProfile(ImageCms.createProfile("sRGB")).tobytes()
    path.write_bytes(profile)
    return profile


def main() -> None:
    ROOT.mkdir(parents=True, exist_ok=True)
    rgb = bytes([255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 0])
    gray = bytes([0, 85, 170, 255])
    cmyk = bytes([0, 255, 255, 0, 255, 0, 255, 0, 255, 255, 0, 0, 0, 0, 255, 0])

    write_pdf("rgb-jpeg.pdf", base_objects(jpeg_image()))
    rgb_large = bytearray()
    for y in range(40):
        for x in range(40):
            rgb_large.extend(
                (255, 0, 0)
                if x < 20 and y < 20
                else (0, 255, 0)
                if x >= 20 and y < 20
                else (0, 0, 255)
                if x < 20
                else (255, 255, 0)
            )
    write_pdf(
        "flate-rgb.pdf",
        base_objects(flate_image(40, 40, b"/DeviceRGB", bytes(rgb_large))),
    )
    write_pdf("flate-gray.pdf", base_objects(flate_image(2, 2, b"/DeviceGray", gray)))
    write_pdf(
        "cmyk.pdf",
        base_objects(
            flate_image(
                2,
                2,
                b"/DeviceCMYK",
                cmyk,
                b"/Decode [1 0 1 0 1 0 1 0]",
            )
        ),
    )

    indexed = stream(
        b"/Type /XObject /Subtype /Image /Width 2 /Height 2 /ColorSpace [/Indexed /DeviceRGB 1 <ff000000ff00>] /BitsPerComponent 8 /Filter /FlateDecode",
        zlib.compress(bytes([0, 1, 1, 0])),
    )
    write_pdf("indexed.pdf", base_objects(indexed))

    profile = srgb_profile()
    icc_image = stream(
        b"/Type /XObject /Subtype /Image /Width 2 /Height 2 /ColorSpace [/ICCBased 7 0 R] /BitsPerComponent 8 /Filter /FlateDecode",
        zlib.compress(rgb),
    )
    icc_objects = base_objects(icc_image)
    icc_objects.append(stream(b"/N 3 /Filter /FlateDecode", zlib.compress(profile)))
    write_pdf("icc-based.pdf", icc_objects)

    smask_image = flate_image(2, 2, b"/DeviceRGB", rgb, b"/SMask 7 0 R")
    smask_objects = base_objects(smask_image)
    smask_objects.append(flate_image(2, 2, b"/DeviceGray", bytes([0, 85, 170, 255])))
    write_pdf("smask.pdf", smask_objects)

    shared_mask_content = b"BT /F1 12 Tf 20 90 Td (PicTrim PDF text) Tj ET q 40 0 0 40 20 20 cm /Im1 Do Q q 40 0 0 40 70 20 cm /Im2 Do Q"
    shared_mask_objects = base_objects(smask_image, shared_mask_content)
    shared_mask_objects[2] = b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 140 120] /Resources << /XObject << /Im1 5 0 R /Im2 8 0 R >> /Font << /F1 6 0 R >> >> /Contents 4 0 R >>"
    shared_mask_objects.append(flate_image(2, 2, b"/DeviceGray", bytes([0, 85, 170, 255])))
    shared_mask_objects.append(flate_image(2, 2, b"/DeviceRGB", rgb, b"/SMask 7 0 R"))
    write_pdf("shared-smask.pdf", shared_mask_objects)

    duplicate_content = b"BT /F1 12 Tf 20 90 Td (PicTrim PDF text) Tj ET q 40 0 0 40 20 20 cm /Im1 Do Q q 20 0 0 20 70 20 cm /Im1 Do Q"
    write_pdf("repeated-reference.pdf", base_objects(flate_image(2, 2, b"/DeviceRGB", rgb), duplicate_content))

    nested = base_objects(flate_image(2, 2, b"/DeviceRGB", rgb), b"BT /F1 12 Tf 20 90 Td (PicTrim PDF text) Tj ET q 40 0 0 40 20 20 cm /Fm1 Do Q")
    nested[2] = b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 140 120] /Resources << /XObject << /Fm1 7 0 R >> /Font << /F1 6 0 R >> >> /Contents 4 0 R >>"
    nested.append(stream(b"/Type /XObject /Subtype /Form /BBox [0 0 1 1] /Resources << /XObject << /Im1 5 0 R >> >>", b"q 1 0 0 1 0 0 cm /Im1 Do Q"))
    write_pdf("nested-form.pdf", nested)

    inline_content = b"BT /F1 12 Tf 20 90 Td (PicTrim PDF text) Tj ET q 40 0 0 40 20 20 cm BI /W 1 /H 1 /CS /RGB /BPC 8 ID \xff\x00\x00 EI Q"
    inline_objects = base_objects(flate_image(1, 1, b"/DeviceRGB", b"\x00\x00\x00"), inline_content)
    inline_objects[2] = b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 140 120] /Resources << /Font << /F1 6 0 R >> >> /Contents 4 0 R >>"
    write_pdf("inline-image.pdf", inline_objects)

    unsupported = stream(
        b"/Type /XObject /Subtype /Image /Width 2 /Height 2 /ColorSpace /DeviceGray /BitsPerComponent 1 /Filter /JBIG2Decode",
        b"not-jbig2",
    )
    write_pdf("unsupported-jbig2.pdf", base_objects(unsupported))
    corrupt = stream(
        b"/Type /XObject /Subtype /Image /Width 2 /Height 2 /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /FlateDecode",
        b"corrupt-flate",
    )
    write_pdf("corrupt-stream.pdf", base_objects(corrupt))

    signed_catalog = b"<< /Type /Catalog /Pages 2 0 R /AcroForm << /Fields [7 0 R] >> >>"
    signed_objects = base_objects(flate_image(2, 2, b"/DeviceRGB", rgb), catalog=signed_catalog)
    signed_objects.append(b"<< /FT /Sig /T (Signature1) >>")
    signed = write_pdf("signed-field.pdf", signed_objects)

    encrypt(signed, "encrypted-empty-password.pdf", "")
    encrypt(signed, "encrypted-password-required.pdf", "secret")


if __name__ == "__main__":
    main()
