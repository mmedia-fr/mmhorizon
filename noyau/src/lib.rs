// SPDX-License-Identifier: GPL-3.0-or-later
//! Noyau de calcul de MMhorizon : d'un panorama 360° à la zone de ciel observable.
//!
//! Il ne dépend pas de Qt. Il sert l'application, l'outil en ligne de commande
//! `mmhorizon-calcul`, et tout programme qui voudrait produire ou lire un
//! fichier `.horizon.json` (spécification : `docs/FORMAT.md`).
//!
//! Chaîne de traitement : [`metadonnees`] (EXIF, XMP GPano) → [`geometrie`]
//! (pixel ↔ azimut et hauteur) → [`detection`] (limite ciel / obstacles,
//! recouvrement du tour) → [`releve`] (profil par degré d'azimut) → [`format`]
//! et [`exports`]. [`soleil`] oriente le panorama sur le soleil qu'il montre.

pub mod controle;
pub mod detection;
pub mod exports;
pub mod format;
pub mod geometrie;
pub mod metadonnees;
pub mod releve;
pub mod soleil;
pub mod synthese;
mod zip_simple;

pub use format::{FichierHorizon, Site};
pub use releve::Releve;

/// Version du noyau, inscrite dans les fichiers produits.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Arrondi à `decimales` chiffres après la virgule.
pub(crate) fn arrondi(valeur: f64, decimales: i32) -> f64 {
    let facteur = 10f64.powi(decimales);
    (valeur * facteur).round() / facteur
}
