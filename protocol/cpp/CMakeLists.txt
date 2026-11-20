# SPDX-License-Identifier: MIT
# The C++ binding of the protocol: one header, and nlohmann/json for the lines of JSON.
cmake_minimum_required(VERSION 3.24)
project(brep_protocol VERSION 1.1.0 LANGUAGES CXX)

# An installed nlohmann/json where there is one; otherwise its release, fetched by its hash.
find_package(nlohmann_json 3.11 QUIET)
if(NOT nlohmann_json_FOUND)
    include(FetchContent)
    FetchContent_Declare(
        nlohmann_json
        URL https://github.com/nlohmann/json/releases/download/v3.11.3/json.tar.xz
        URL_HASH SHA256=d6c65aca6b1ed68e7a182f4757257b107ae403032760ed6ef121c9d55e81757d
        DOWNLOAD_EXTRACT_TIMESTAMP TRUE
    )
    FetchContent_MakeAvailable(nlohmann_json)
endif()

add_library(brep_protocol INTERFACE)
add_library(brep_protocol::brep_protocol ALIAS brep_protocol)
target_include_directories(brep_protocol INTERFACE ${CMAKE_CURRENT_SOURCE_DIR}/include)
target_link_libraries(brep_protocol INTERFACE nlohmann_json::nlohmann_json)
target_compile_features(brep_protocol INTERFACE cxx_std_17)

# The example and the tests, when this is the project being built and not a dependency.
if(CMAKE_SOURCE_DIR STREQUAL CMAKE_CURRENT_SOURCE_DIR)
    if(NOT MSVC)
        add_compile_options(-Wall -Wextra -Werror)
    endif()
    add_executable(box_kernel examples/box_kernel.cpp)
    target_link_libraries(box_kernel PRIVATE brep_protocol)

    # The tests read doc/api.md, so they build in the repository and not from a release's archive.
    if(EXISTS ${CMAKE_CURRENT_SOURCE_DIR}/tests/serve_test.cpp)
        enable_testing()
        # The adapter protocol/parity/compare.py runs against the Rust binding's twin.
        add_executable(echo_kernel tests/echo_kernel.cpp)
        target_link_libraries(echo_kernel PRIVATE brep_protocol)
        add_executable(serve_test tests/serve_test.cpp)
        target_link_libraries(serve_test PRIVATE brep_protocol)
        target_compile_definitions(serve_test PRIVATE
                                   API_MD="${CMAKE_CURRENT_SOURCE_DIR}/../../doc/api.md")
        add_test(NAME serve_test COMMAND serve_test)
    endif()
endif()
