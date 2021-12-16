vcpkg_download_distfile(ARCHIVE
    URLS "https://github.com/phoerious/lexbor/archive/ac4d36e2a6a8570d20f949f2f6101a7e1e3d6d33.zip"
    FILENAME "v2.1.0git.zip"
    SHA512 aed80748e3e973a670367e5b8ff273d861c2077a34bf62bc62d2a6a460fa5106b734d4aaf1b60e3f38f04455616c23174a8481a205f994bcf0ed685310b1662e
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

