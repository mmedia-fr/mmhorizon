// SPDX-License-Identifier: GPL-3.0-or-later
//! Ce que l'interface doit savoir du programme lui-même.

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, noyau)]
        /// Version du programme, telle que la porte le noyau.
        #[qproperty(QString, version)]
        type Socle = super::SocleRust;

        #[qinvokable]
        #[cxx_name = "plateforme"]
        fn plateforme(&self) -> QString;
    }
}

use cxx_qt_lib::QString;

pub struct SocleRust {
    noyau: QString,
    version: QString,
}

impl Default for SocleRust {
    fn default() -> Self {
        Self {
            noyau: QString::from(&format!("mmhorizon_core {}", env!("CARGO_PKG_VERSION"))),
            version: QString::from(env!("CARGO_PKG_VERSION")),
        }
    }
}

impl qobject::Socle {
    pub fn plateforme(&self) -> QString {
        QString::from(&format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH))
    }
}
