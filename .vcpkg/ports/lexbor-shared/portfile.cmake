vcpkg_download_distfile(ARCHIVE
    URLS "https://github.com/lexbor/lexbor/archive/refs/tags/v2.2.0.tar.gz"
    FILENAME "v2.2.0"
    SHA512 26bbca3b41a417cbc59ba8cf736e1611966fc2202de85aabf621b840565d835e7e5ffc1b0294defc16ec883f9fb94e802bd19ed704be35fa79b41566acc05cbc
)

vcpkg_extract_source_archive_ex(
    OUT_SOURCE_PATH SOURCE_PATH
    ARCHIVE ${ARCHIVE}
)

vcpkg_configure_cmake(
    SOURCE_PATH ${SOURCE_PATH}
    PREFER_NINJA
)

vcpkg_install_cmake()

# Handle copyright
file(INSTALL ${SOURCE_PATH}/LICENSE DESTINATION ${CURRENT_PACKAGES_DIR}/share/lexbor RENAME copyright)

# Delete empty folders and duplicate files
file(REMOVE_RECURSE "${CURRENT_PACKAGES_DIR}/include/lexbor/html/tree/insertion_mode")
file(REMOVE_RECURSE "${CURRENT_PACKAGES_DIR}/debug/include")
