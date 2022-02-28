vcpkg_download_distfile(ARCHIVE
    URLS "https://github.com/phoerious/lexbor/archive/aea5d9836edf05da595b3bd1d413f3ebdfb04a15.zip"
    FILENAME "v2.1.0git.zip"
    SHA512 79d0e55b0fd8b41301adf02674a737c025507329da6c2dffb5d287da338ae02dd8d90f601733fab3c2979a41bf48a4edc41d311b9d11cf276a4239f3b52982ec
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

