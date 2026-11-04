vcpkg_from_github(
    OUT_SOURCE_PATH SOURCE_PATH
    REPO lexbor/lexbor
    # Upstream master, pinned for reproducible builds.
    REF 145872a2e5d6c22d923afcd3c386a21e59f9a0e2
    SHA512 95eaeb19c75529a6091045fc460117046ec72e8ceaa6bd8293b17b6a2603459e48c94dc905ce9b0db9742d84b430adbfc6c0efd4e103986ce46a3bff351814d3
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
