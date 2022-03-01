vcpkg_download_distfile(ARCHIVE
    URLS "https://github.com/lexbor/lexbor/archive/9c6915b98b0c33ac140bc85cc2c9b9eb0a93751b.zip"
    FILENAME "v2.1.0git.zip"
    SHA512 fefae72903fdff2a8f771d6909e683c150357debda66b55f992a26d4ca4fbca67eac2d67f362a9d49e6b85f49b732de95c5f27bc8078d0df3f947f1e596632a1
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

