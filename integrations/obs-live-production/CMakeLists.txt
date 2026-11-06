cmake_minimum_required(VERSION 3.22)

project(obs-live-production VERSION 0.1.0 LANGUAGES CXX)

include(CTest)

find_package(libobs REQUIRED)
find_package(obs-frontend-api REQUIRED)
find_package(Threads REQUIRED)
find_package(Qt6 REQUIRED COMPONENTS Network Widgets)

add_library(obs-live-production MODULE
  src/config.cpp
  src/fixture.cpp
  src/fixture_config.cpp
  src/fixture_dispatcher.cpp
  src/fixture_prepare.cpp
  src/fixture_source.cpp
  src/fixture_state.cpp
  src/frontend.cpp
  src/frontend_dialog.cpp
  src/module_runtime.cpp
  src/module_runtime_actions.cpp
  src/module_runtime_poll.cpp
  src/plugin.cpp
  src/publisher.cpp
  src/publisher_runtime.cpp
  src/recovery_action.cpp
  src/recovery_client.cpp
  src/recovery_fault.cpp
  src/recovery_fixture.cpp
  src/recovery_model.cpp
  src/recovery_protocol.cpp
  src/recovery_store.cpp
  src/report.cpp
  src/srt_url.cpp
)

target_compile_features(obs-live-production PRIVATE cxx_std_17)
target_compile_options(obs-live-production PRIVATE
  $<$<CXX_COMPILER_ID:GNU,Clang>:-Wall -Wextra -Wpedantic -Werror>
)
target_link_libraries(obs-live-production PRIVATE
  OBS::libobs
  OBS::obs-frontend-api
  Qt6::Widgets
  Qt6::Network
  Threads::Threads
)
set_target_properties(obs-live-production PROPERTIES PREFIX "")

if(BUILD_TESTING)
  add_executable(obs-live-production-fixture-audio-test
    tests/fixture_audio_test.cpp src/fixture_source.cpp)
  target_compile_features(obs-live-production-fixture-audio-test PRIVATE cxx_std_17)
  target_compile_options(obs-live-production-fixture-audio-test PRIVATE
    $<$<CXX_COMPILER_ID:GNU,Clang>:-Wall -Wextra -Wpedantic -Werror>)
  target_include_directories(obs-live-production-fixture-audio-test PRIVATE src)
  target_link_libraries(obs-live-production-fixture-audio-test PRIVATE OBS::libobs Threads::Threads)
  add_test(NAME obs-live-production-fixture-audio-test COMMAND obs-live-production-fixture-audio-test)
  set_tests_properties(obs-live-production-fixture-audio-test PROPERTIES TIMEOUT 20)
  add_executable(obs-live-production-srt-url-test
    tests/srt_url_test.cpp
    src/srt_url.cpp
  )
  target_compile_features(obs-live-production-srt-url-test PRIVATE cxx_std_17)
  target_compile_options(obs-live-production-srt-url-test PRIVATE
    $<$<CXX_COMPILER_ID:GNU,Clang>:-Wall -Wextra -Wpedantic -Werror>
  )
  target_include_directories(obs-live-production-srt-url-test PRIVATE src)
  add_test(NAME obs-live-production-srt-url-test
    COMMAND obs-live-production-srt-url-test
  )
  add_executable(obs-live-production-fixture-state-test
    tests/fixture_state_test.cpp
    src/fixture_state.cpp
  )
  target_compile_features(obs-live-production-fixture-state-test PRIVATE cxx_std_17)
  target_compile_options(obs-live-production-fixture-state-test PRIVATE
    $<$<CXX_COMPILER_ID:GNU,Clang>:-Wall -Wextra -Wpedantic -Werror>
  )
  target_include_directories(obs-live-production-fixture-state-test PRIVATE src)
  target_link_libraries(obs-live-production-fixture-state-test PRIVATE Threads::Threads)
  add_test(NAME obs-live-production-fixture-state-test
    COMMAND obs-live-production-fixture-state-test
  )
  add_executable(obs-live-production-fixture-dispatcher-test
    tests/fixture_dispatcher_test.cpp
    src/fixture_dispatcher.cpp
  )
  target_compile_features(obs-live-production-fixture-dispatcher-test PRIVATE cxx_std_17)
  target_compile_definitions(obs-live-production-fixture-dispatcher-test PRIVATE
    OBS_LIVE_PRODUCTION_DISPATCHER_TESTING
  )
  target_compile_options(obs-live-production-fixture-dispatcher-test PRIVATE
    $<$<CXX_COMPILER_ID:GNU,Clang>:-Wall -Wextra -Wpedantic -Werror>
  )
  target_include_directories(obs-live-production-fixture-dispatcher-test PRIVATE src)
  target_link_libraries(obs-live-production-fixture-dispatcher-test PRIVATE
    Qt6::Core
    Threads::Threads
  )
  add_test(NAME obs-live-production-fixture-dispatcher-test
    COMMAND obs-live-production-fixture-dispatcher-test
  )
  add_executable(obs-live-production-recovery-model-test
    tests/recovery_model_test.cpp
    src/recovery_model.cpp
  )
  target_compile_features(obs-live-production-recovery-model-test PRIVATE cxx_std_17)
  target_compile_options(obs-live-production-recovery-model-test PRIVATE
    $<$<CXX_COMPILER_ID:GNU,Clang>:-Wall -Wextra -Wpedantic -Werror>
  )
  target_include_directories(obs-live-production-recovery-model-test PRIVATE src)
  add_test(NAME obs-live-production-recovery-model-test
    COMMAND obs-live-production-recovery-model-test
  )
  add_executable(obs-live-production-recovery-action-test
    tests/recovery_action_test.cpp
    src/recovery_action.cpp
    src/recovery_model.cpp
  )
  add_executable(obs-live-production-recovery-fault-test
    tests/recovery_fault_test.cpp
    src/recovery_fault.cpp
  )
  target_compile_features(obs-live-production-recovery-fault-test PRIVATE cxx_std_17)
  target_compile_options(obs-live-production-recovery-fault-test PRIVATE
    $<$<CXX_COMPILER_ID:GNU,Clang>:-Wall -Wextra -Wpedantic -Werror>
  )
  target_include_directories(obs-live-production-recovery-fault-test PRIVATE src)
  add_test(NAME obs-live-production-recovery-fault-test
    COMMAND obs-live-production-recovery-fault-test
  )
  target_compile_features(obs-live-production-recovery-action-test PRIVATE cxx_std_17)
  target_compile_options(obs-live-production-recovery-action-test PRIVATE
    $<$<CXX_COMPILER_ID:GNU,Clang>:-Wall -Wextra -Wpedantic -Werror>
  )
  target_include_directories(obs-live-production-recovery-action-test PRIVATE src)
  add_test(NAME obs-live-production-recovery-action-test
    COMMAND obs-live-production-recovery-action-test
  )
  add_executable(obs-live-production-recovery-store-test
    tests/recovery_store_test.cpp
    src/recovery_model.cpp
    src/recovery_store.cpp
  )
  target_compile_features(obs-live-production-recovery-store-test PRIVATE cxx_std_17)
  target_compile_options(obs-live-production-recovery-store-test PRIVATE
    $<$<CXX_COMPILER_ID:GNU,Clang>:-Wall -Wextra -Wpedantic -Werror>
  )
  target_include_directories(obs-live-production-recovery-store-test PRIVATE src)
  add_test(NAME obs-live-production-recovery-store-test
    COMMAND obs-live-production-recovery-store-test
  )
  add_executable(obs-live-production-recovery-protocol-test
    tests/recovery_protocol_test.cpp
    src/recovery_protocol.cpp
  )
  target_compile_features(obs-live-production-recovery-protocol-test PRIVATE cxx_std_17)
  target_compile_options(obs-live-production-recovery-protocol-test PRIVATE
    $<$<CXX_COMPILER_ID:GNU,Clang>:-Wall -Wextra -Wpedantic -Werror>
  )
  target_include_directories(obs-live-production-recovery-protocol-test PRIVATE src)
  target_link_libraries(obs-live-production-recovery-protocol-test PRIVATE Qt6::Core)
  add_test(NAME obs-live-production-recovery-protocol-test
    COMMAND obs-live-production-recovery-protocol-test
  )
  add_executable(obs-live-production-recovery-client-test
    tests/recovery_client_test.cpp
    src/recovery_client.cpp
    src/recovery_protocol.cpp
  )
  target_compile_features(obs-live-production-recovery-client-test PRIVATE cxx_std_17)
  target_compile_options(obs-live-production-recovery-client-test PRIVATE
    $<$<CXX_COMPILER_ID:GNU,Clang>:-Wall -Wextra -Wpedantic -Werror>
  )
  target_include_directories(obs-live-production-recovery-client-test PRIVATE src)
  target_link_libraries(obs-live-production-recovery-client-test PRIVATE Qt6::Network)
  add_test(NAME obs-live-production-recovery-client-test
    COMMAND obs-live-production-recovery-client-test
  )
  add_executable(obs-live-production-module-runtime-test
    tests/module_runtime_test.cpp
    src/config.cpp
    src/fixture_dispatcher.cpp
    src/module_runtime.cpp
    src/module_runtime_actions.cpp
    src/module_runtime_poll.cpp
    src/publisher.cpp
    src/publisher_runtime.cpp
    src/recovery_action.cpp
    src/recovery_client.cpp
    src/recovery_model.cpp
    src/recovery_protocol.cpp
    src/recovery_store.cpp
    src/srt_url.cpp
  )
  target_compile_features(obs-live-production-module-runtime-test PRIVATE cxx_std_17)
  target_compile_options(obs-live-production-module-runtime-test PRIVATE
    $<$<CXX_COMPILER_ID:GNU,Clang>:-Wall -Wextra -Wpedantic -Werror>
  )
  target_include_directories(obs-live-production-module-runtime-test PRIVATE src)
  target_link_libraries(obs-live-production-module-runtime-test PRIVATE
    OBS::libobs
    Qt6::Network
    Threads::Threads
  )
  add_test(NAME obs-live-production-module-runtime-test
    COMMAND obs-live-production-module-runtime-test
  )
endif()

install(TARGETS obs-live-production
  LIBRARY DESTINATION "bin/64bit"
)
install(DIRECTORY data/
  DESTINATION "data/obs-plugins/obs-live-production"
)
install(DIRECTORY schemas/
  DESTINATION "data/obs-plugins/obs-live-production/schemas"
)
