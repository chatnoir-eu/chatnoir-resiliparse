vcpkg_download_distfile(ARCHIVE
    URLS "https://github.com/lexbor/lexbor/archive/a94adb86b13a36f7062ab79169a520a4b8186173.zip"
    FILENAME "v2.1.0git.zip"
    SHA512 b1a10d93e6659a5174d960d8c119e7f1a97e123b314fb96ded38c97bbb693c1c984caaf13eef017b6c21856c9879dce1fead45a064cb849678ff3bff36d476ef
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

