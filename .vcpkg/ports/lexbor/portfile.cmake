vcpkg_download_distfile(ARCHIVE
    URLS "https://github.com/lexbor/lexbor/archive/31270b1dfe8851f3779ca13d6efaae128178925f.zip"
    FILENAME "v2.1.0git.zip"
    SHA512 9849bd13f289ee907dc890a3f11ec73b552bf57054b6a07f0c9d2319026c88b416a710c3bfc452c4ede146689c192cc1cd52115b8e8ead085cb13ba548643a6b
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

