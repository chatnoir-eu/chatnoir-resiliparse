vcpkg_from_github(
    OUT_SOURCE_PATH SOURCE_PATH
    REPO lexbor/lexbor
    REF v${VERSION}
    SHA512 d65906504f490a03579eb737fa85007f1e948c4e82f986454d763728e4d71be40fe953a5a7cfda84b322cb99837f482295ee89bc3f07f2ef89ccaeeab39a4acf
    PATCHES
        patches/0001-reference-counting.patch
        patches/0002-static-library-name.patch
)

string(COMPARE EQUAL "${VCPKG_LIBRARY_LINKAGE}" "static" BUILD_STATIC)
string(COMPARE EQUAL "${VCPKG_LIBRARY_LINKAGE}" "dynamic" BUILD_SHARED)

vcpkg_cmake_configure(
    SOURCE_PATH "${SOURCE_PATH}"
    OPTIONS
    -DLEXBOR_BUILD_SHARED=${BUILD_SHARED}
    -DLEXBOR_BUILD_STATIC=${BUILD_STATIC}
    -DLEXBOR_OPTIMIZATION_LEVEL=-O3
)
vcpkg_cmake_install()
vcpkg_copy_pdbs()

file(REMOVE_RECURSE "${CURRENT_PACKAGES_DIR}/debug/include"
    "${CURRENT_PACKAGES_DIR}/include/lexbor/html/tree/insertion_mode"
    "${CURRENT_PACKAGES_DIR}/debug/include/lexbor/html/tree/insertion_mode"
)

vcpkg_install_copyright(FILE_LIST "${SOURCE_PATH}/LICENSE")
