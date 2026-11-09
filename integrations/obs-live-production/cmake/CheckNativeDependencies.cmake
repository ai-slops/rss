# Loading both Ubuntu SRT TLS variants can crash their global destructors.
find_program(LDD_EXECUTABLE ldd REQUIRED)
execute_process(COMMAND "${LDD_EXECUTABLE}" "${PLUGIN_BINARY}"
  RESULT_VARIABLE result OUTPUT_VARIABLE dependencies ERROR_VARIABLE detail)
if(NOT result EQUAL 0 OR dependencies MATCHES "not found")
  message(FATAL_ERROR "Plugin dependencies did not resolve: ${dependencies} ${detail}")
endif()
string(REGEX MATCHALL "libsrt(-gnutls|-openssl)?\\.so[.0-9]*" srt_libraries "${dependencies}")
list(REMOVE_DUPLICATES srt_libraries)
list(LENGTH srt_libraries runtime_count)
if(runtime_count GREATER 1)
  message(FATAL_ERROR
    "Conflicting SRT runtimes (${srt_libraries}). Use the same SRT variant as FFmpeg; on Ubuntu 24.04 install libsrt-gnutls-dev libgnutls28-dev nettle-dev, then rebuild in a fresh build directory.")
endif()
