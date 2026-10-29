cmake_minimum_required(VERSION 3.31.6)
project(serea_local_model LANGUAGES C CXX)

if(NOT DEFINED LLAMA_SRC OR NOT EXISTS "${LLAMA_SRC}/CMakeLists.txt")
    message(FATAL_ERROR "LLAMA_SRC must point to the pinned llama.cpp checkout")
endif()
find_package(Git REQUIRED)
execute_process(
    COMMAND "${GIT_EXECUTABLE}" -C "${LLAMA_SRC}" rev-parse HEAD
    OUTPUT_VARIABLE LLAMA_SOURCE_HEAD
    OUTPUT_STRIP_TRAILING_WHITESPACE
    RESULT_VARIABLE LLAMA_SOURCE_HEAD_RESULT
)
if(NOT LLAMA_SOURCE_HEAD_RESULT EQUAL 0 OR
   NOT LLAMA_SOURCE_HEAD STREQUAL "276378515ceebee5d14bcc8536603dd5c87491a2")
    message(FATAL_ERROR "LLAMA_SRC must be llama.cpp revision 276378515ceebee5d14bcc8536603dd5c87491a2")
endif()
execute_process(
    COMMAND "${GIT_EXECUTABLE}" -C "${LLAMA_SRC}" status --porcelain=v1 --untracked-files=all --ignore-submodules=none
    OUTPUT_VARIABLE LLAMA_SOURCE_WORKTREE_STATUS
    OUTPUT_STRIP_TRAILING_WHITESPACE
    RESULT_VARIABLE LLAMA_SOURCE_WORKTREE_STATUS_RESULT
)
if(NOT LLAMA_SOURCE_WORKTREE_STATUS_RESULT EQUAL 0 OR
   NOT LLAMA_SOURCE_WORKTREE_STATUS STREQUAL "")
    message(FATAL_ERROR "LLAMA_SRC must be a clean Git worktree; tracked or untracked changes are not accepted")
endif()

set(BUILD_SHARED_LIBS OFF CACHE BOOL "" FORCE)
set(LLAMA_BUILD_TESTS OFF CACHE BOOL "" FORCE)
set(LLAMA_BUILD_TOOLS OFF CACHE BOOL "" FORCE)
set(LLAMA_BUILD_EXAMPLES OFF CACHE BOOL "" FORCE)
set(LLAMA_BUILD_SERVER OFF CACHE BOOL "" FORCE)
set(LLAMA_BUILD_COMMON OFF CACHE BOOL "" FORCE)
set(LLAMA_BUILD_APP OFF CACHE BOOL "" FORCE)
set(LLAMA_OPENSSL OFF CACHE BOOL "" FORCE)
set(GGML_NATIVE OFF CACHE BOOL "" FORCE)
set(GGML_OPENMP OFF CACHE BOOL "" FORCE)
set(GGML_LLAMAFILE OFF CACHE BOOL "" FORCE)
set(GGML_BACKEND_DL OFF CACHE BOOL "" FORCE)
set(GGML_CPU_KLEIDIAI OFF CACHE BOOL "" FORCE)

add_subdirectory("${LLAMA_SRC}" "${CMAKE_BINARY_DIR}/llama")

add_library(serea_local_llama SHARED llama_jni.cpp)
target_compile_features(serea_local_llama PRIVATE cxx_std_17)
target_include_directories(serea_local_llama PRIVATE "${LLAMA_SRC}/include")
target_link_libraries(serea_local_llama PRIVATE llama log android)
option(SEREA_NATIVE_JNI_TEST_SEAMS "Enable failure injection only in the isolated native runtime diagnostic" OFF)
if(SEREA_NATIVE_JNI_TEST_SEAMS)
    target_compile_definitions(serea_local_llama PRIVATE SEREA_NATIVE_JNI_TEST_SEAMS=1)
endif()
