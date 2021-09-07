vcpkg_download_distfile(ARCHIVE
    URLS "https://github.com/phoerious/lexbor/archive/58fba73a30cabbf8d8dae3ac1c22a7e15c0c53c2.zip"
    FILENAME "v2.1.0git.zip"
    SHA512 45d9db5dc3dd1ef979e4f9336578671709038028e84f5a86dac95666beb1e76f8fcf0afe36a9b432f80a97096cf263f5071667e56dc59c249897a5d1777db40c
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

