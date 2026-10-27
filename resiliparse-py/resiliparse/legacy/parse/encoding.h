/*
    Copyright 2021 Janek Bevendorff

    Licensed under the Apache License, Version 2.0 (the "License");
    you may not use this file except in compliance with the License.
    You may obtain a copy of the License at

        http://www.apache.org/licenses/LICENSE-2.0

    Unless required by applicable law or agreed to in writing, software
    distributed under the License is distributed on an "AS IS" BASIS,
    WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
    See the License for the specific language governing permissions and
    limitations under the License.
*/

#ifndef RESILIPARSE_ENCODING_H
#define RESILIPARSE_ENCODING_H

#include <string>
#include <vector>

typedef struct {
    const std::string magic_bytes;
    const std::string mime_type;
} mime_bytes_t;

using namespace std::literals::string_literals;

static std::vector<mime_bytes_t> MIME_BYTES = {
    {"\xEF\xBB\xBF"s, "text/plain"},
    {"\xFF\xFE"s, "text/plain"},
    {"\xFE\xFF"s, "text/plain"},
    {"\x0E\xFE\xFF"s, "text/plain"},
    {"\x2B\x2F\x76\x38"s, "text/plain"},
    {"\x2B\x2F\x76\x39"s, "text/plain"},
    {"\x2B\x2F\x76\x2B"s, "text/plain"},
    {"\x2B\x2F\x76\x2F"s, "text/plain"},
    {"<!DOCTYPE html"s, "text/html"},
    {"<!DOCTYPE HTML"s, "text/html"},
    {"<!doctype html"s, "text/html"},
    {"<!doctype HTML"s, "text/html"},
    {"<!DOCTYPE svg"s, "image/svg+xml"},
    {"<!doctype svg"s, "image/svg+xml"},
    {"<!DOCTYPE SVG"s, "image/svg+xml"},
    {"<!doctype SVG"s, "image/svg+xml"},
    {"<?xml "s, "application/xml"},
    {"\x00<\x00?\x00x\x00m\x00l\x00 "s, "application/xml"},
    {"{\\rtf1"s, "application/rtf"},
    {"\xD0\xCF\x11\xE0\xA1\xB1\x1A\xE1"s, "application/msword"},

    {"\xFF\xD8\xFF\xE0\x00\x10\x4A\x46\x49\x46\x00\x01"s, "image/jpeg"},
    {"\xFF\xD8\xFF\xE0"s, "image/jpeg"},
    {"\xFF\xD8\xFF\xE1"s, "image/jpeg"},
    {"\xFF\xD8\xFF\xE2"s, "image/jpeg"},
    {"\xFF\xD8\xFF\xE8"s, "image/jpeg"},
    {"\xFF\xD8\xFF\xEE"s, "image/jpeg"},
    {"\x89\x50\x4E\x47\x0D\x0A\x1A\x0A"s, "image/png"},
    {"\x47\x49\x46\x38\x37\x61"s, "image/gif"},
    {"\x49\x49\x2A\x00"s, "image/tiff"},
    {"\x4D\x4D\x00\x2A"s, "image/tiff"},
    {"\x00\x00\x01\x00"s, "image/x-icon"},
    {"\x69\x63\x6e\x73"s, "image/icns"},
    {"\x00\x00\x00\x0C\x6A\x50\x20\x20\x0D\x0A\x87\x0A"s, "image/jp2"},
    {"\xFF\x4F\xFF\x51"s, "image/jp2"},
    {"\x42\x4D"s, "image/bmp"},
    {"\x52\x49\x46\x46"s, "image/webp"},
    {"\x2F\x2A\x20\x58\x50\x4D\x20\x2A\x2F"s, "image/x-xpixmap"},

    {"\x25\x50\x44\x46\x2D"s, "application/pdf"},
    {"\x25\x21\x50\x53"s, "application/postscript"},
    {"\x38\x42\x50\x53"s, "image/vnd.adobe.photoshop"},

    {"\x50\x4B\x03\x04"s, "application/zip"},
    {"\x50\x4B\x05\x06"s, "application/zip"},
    {"\x50\x4B\x07\x08"s, "application/zip"},
    {"\x75\x73\x74\x61\x72\x00\x30\x30"s, "application/x-tar"},
    {"\x75\x73\x74\x61\x72\x20\x20\x00"s, "application/x-tar"},
    {"\x37\x7A\xBC\xAF\x27\x1C"s, "application/x-7z-compressed"},
    {"\x1F\x8B"s, "application/gzip"},
    {"\x49\x4E\x44\x58"s, "application/x-bzip2"},
    {"\x42\x5A\x68"s, "application/x-bzip2"},
    {"\x04\x22\x4D\x18"s, "application/x-lz4"},
    {"\xFD\x37\x7A\x58\x5A\x00"s, "application/x-xz"},
    {"\x52\x61\x72\x21\x1A\x07\x00"s, "application/vnd.rar"},
    {"\x52\x61\x72\x21\x1A\x07\x01\x00"s, "application/vnd.rar"},

    {"\x77\x4F\x46\x46"s, "font/woff"},
    {"\x77\x4F\x46\x32"s, "font/woff2"},
    {"\x00\x01\x00\x00\x00"s, "font/ttf"},
    {"\x4B\x43\x4D\x53"s, "application/vnd.iccprofile"},

    {"\x4F\x67\x67\x53"s, "application/ogg"},
    {"\x66\x4C\x61\x43"s, "audio/flac"},
    {"\x46\x4F\x52\x4D"s, "audio/aiff"},
    {"\x52\x49\x46\x46"s, "audio/wav"},
    {"\xFF\xFB"s, "audio/mpeg"},
    {"\xFF\xF2"s, "audio/mpeg"},
    {"\xFF\xF2"s, "audio/mpeg"},
    {"\x49\x44\x33"s, "audio/mpeg"},
    {"\x66\x74\x79\x70\x69\x73\x6F\x6D"s, "video/mp4"},
    {"\x00\x00\x01\xB3"s, "video/mpeg"},
    {"\x1A\x45\xDF\xA3"s, "video/x-matroska"},
    {"\x52\x49\x46\x46"s, "video/x-msvideo"},
    {"\x00\x00\x01\xBA"s, "video/mpeg"},
    {"\x43\x57\x53"s, "application/x-shockwave-flash"},
    {"\x46\x57\x53"s, "application/x-shockwave-flash"},

    {"\x7F\x45\x4C\x46"s, "application/x-elf"},
    {"\x4D\x53\x43\x46"s, "application/vnd.ms-cab-compressed"},
    {"\x43\x44\x30\x30\x31"s, "application/x-iso9660-image"},
    {"\xFE\xED\xFA\xCE"s, "application/x-mach-binary"},
    {"\xFE\xED\xFA\xCF"s, "application/x-mach-binary"},
    {"\xCE\xFA\xED\xFE"s, "application/x-mach-binary"},
    {"\xCF\xFA\xED\xFE"s, "application/x-mach-binary"},
    {"\x00\x61\x73\x6D"s, "application/wasm"},
    {"\x21\x3C\x61\x72\x63\x68\x3E\x0A"s, "application/vnd.debian.binary-package"},
    {"\xCA\xFE\xBA\xBE"s, "application/java-vm"},
    {"\x49\x54\x53\x46\x03\x00\x00\x00\x60\x00\x00\x00"s, "application/vnd.ms-htmlhelp"}
};

#endif  // RESILIPARSE_ENCODING_H
