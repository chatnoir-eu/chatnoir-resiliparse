vcpkg_download_distfile(ARCHIVE
    URLS "https://github.com/lexbor/lexbor/archive/bc228aaf98c19a68a74d80e86c893148f8f56b2d.zip"
    FILENAME "v2.1.0git.zip"
    SHA512 3ae9205a97bc17abf8cb96f4f9da2ea36f03691aeeb6ef4611addb82dadd91c4373c4e080b213cc53d637cc34cf0877c273ebb67d6f2acc10464f5bbdbac43af
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

