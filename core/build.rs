// SPDX-License-Identifier: GPL-3.0-or-later
use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    // Le rcc de l'hôte compresse en zstd dès que le gain le justifie, et le Qt
    // officiel pour Android est construit sans zstd : l'édition de liens
    // s'arrêterait sur « undefined symbol: qt_resourceFeatureZstd » (relevé sur
    // MMdedit, même socle).
    std::env::set_var("CXX_QT_AUTORCC_OPTIONS", "--no-zstd");

    CxxQtBuilder::new_qml_module(QmlModule::new("fr.mmedia.mmhorizon").qml_files(["qml/Main.qml"]))
        .qt_module("Qml")
        .files(["src/socle.rs", "src/releve_qt.rs"])
        .build();
}
