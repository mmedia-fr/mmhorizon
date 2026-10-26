# SPDX-License-Identifier: GPL-3.0-or-later
# MMhorizon — noyau de calcul Rust (noyau/), liaison Qt (core/), interface Qt 6 / QML.
cmake_minimum_required(VERSION 3.24)
project(mmhorizon VERSION 0.1.3 LANGUAGES CXX)

if(CMAKE_CXX_COMPILER_ID STREQUAL "MSVC")
  set(CMAKE_MSVC_RUNTIME_LIBRARY "MultiThreadedDLL")
endif()

set(CMAKE_AUTOMOC ON)
set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

find_package(Qt6 6.4 REQUIRED COMPONENTS Core Gui Qml Quick QuickControls2 QuickDialogs2)
if(NOT ANDROID)
  find_package(Qt6 REQUIRED COMPONENTS QmlImportScanner)
endif()
qt_standard_project_setup()

if(MSVC)
  set_property(TARGET Qt6::Core Qt6::Gui Qt6::Qml Qt6::Quick Qt6::QuickControls2 Qt6::QuickDialogs2
               PROPERTY MAP_IMPORTED_CONFIG_DEBUG "RELEASE")
endif()

if(ANDROID)
  if(ANDROID_ABI STREQUAL "arm64-v8a")
    set(Rust_CARGO_TARGET aarch64-linux-android)
  elseif(ANDROID_ABI STREQUAL "x86_64")
    set(Rust_CARGO_TARGET x86_64-linux-android)
  endif()
endif()

find_package(CxxQt QUIET)
if(NOT CxxQt_FOUND)
  include(FetchContent)
  FetchContent_Declare(CxxQt
    GIT_REPOSITORY https://github.com/kdab/cxx-qt-cmake.git
    GIT_TAG 0.10.0)
  FetchContent_MakeAvailable(CxxQt)
endif()

# En compilation croisée, Qt::qmake désigne le qmake de l'hôte : cxx-qt lirait alors
# les en-têtes du Qt de bureau. Sous Android, on lui donne le qmake du Qt cible.
set(MMHORIZON_QMAKE)
if(ANDROID)
  set(MMHORIZON_QMAKE QMAKE "${QT6_INSTALL_PREFIX}/${QT6_INSTALL_BINS}/qmake")
endif()

cxx_qt_import_crate(
  MANIFEST_PATH core/Cargo.toml
  ${MMHORIZON_QMAKE}
  CRATES mmhorizon_core
  LOCKED
  QT_MODULES Qt::Core Qt::Gui Qt::Qml Qt::Quick Qt::QuickControls2)

cxx_qt_import_qml_module(mmhorizon_qml_module
  URI "fr.mmedia.mmhorizon"
  SOURCE_CRATE mmhorizon_core)

# Le .h figure dans les sources pour qu'AUTOMOC le traite.
set(MMHORIZON_SOURCES cpp/main.cpp cpp/fichiers.h)
if(WIN32)
  list(APPEND MMHORIZON_SOURCES build/mmhorizon.rc)
endif()

qt_add_executable(mmhorizon ${MMHORIZON_SOURCES})
# Pas de ressource Qt sous Android : le rcc de l'hôte compresse en zstd, absent du
# Qt officiel pour Android (relevé sur MMdedit). L'icône y est portée par le manifeste.
if(NOT ANDROID)
  qt_add_resources(mmhorizon "ressources"
    PREFIX "/"
    FILES assets/mmhorizon.ico assets/mmhorizon.png)
endif()
target_link_libraries(mmhorizon PRIVATE mmhorizon_qml_module Qt::Quick Qt::QuickControls2
                                        Qt::QuickDialogs2)
target_compile_definitions(mmhorizon PRIVATE MMHORIZON_VERSION="${PROJECT_VERSION}")

set_target_properties(mmhorizon PROPERTIES
  WIN32_EXECUTABLE ON
  MACOSX_BUNDLE ON
  MACOSX_BUNDLE_GUI_IDENTIFIER fr.mmedia.mmhorizon
  MACOSX_BUNDLE_BUNDLE_NAME MMhorizon
  MACOSX_BUNDLE_BUNDLE_VERSION ${PROJECT_VERSION}
  MACOSX_BUNDLE_SHORT_VERSION_STRING ${PROJECT_VERSION}
  QT_ANDROID_PACKAGE_NAME fr.mmedia.mmhorizon
  QT_ANDROID_APP_NAME MMhorizon
  QT_ANDROID_VERSION_NAME ${PROJECT_VERSION})

if(NOT ANDROID)
  qt_import_qml_plugins(mmhorizon)
endif()

if(NOT ANDROID)
  install(TARGETS mmhorizon
    BUNDLE DESTINATION .
    RUNTIME DESTINATION ${CMAKE_INSTALL_BINDIR})
endif()

enable_testing()
add_test(NAME smoke COMMAND mmhorizon --smoke)
set_tests_properties(smoke PROPERTIES ENVIRONMENT "QT_QPA_PLATFORM=offscreen")
