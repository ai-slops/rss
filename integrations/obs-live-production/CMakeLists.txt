cmake_minimum_required(VERSION 3.22)

project(obs-live-production VERSION 0.1.0 LANGUAGES CXX)

include(CTest)

find_package(libobs REQUIRED)
find_package(obs-frontend-api REQUIRED)
find_package(Threads REQUIRED)
find_package(Qt6 REQUIRED COMPONENTS Widgets)

add_library(obs-live-production MODULE
  src/config.cpp
  src/fixture.cpp
  src/fixture_config.cpp
  src/fixture_prepare.cpp
  src/fixture_source.cpp
  src/frontend.cpp
  src/plugin.cpp
  src/publisher.cpp
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
  Threads::Threads
)
set_target_properties(obs-live-production PROPERTIES PREFIX "")

if(BUILD_TESTING)
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
