// SPDX-License-Identifier: GPL-3.0-or-later
//! Correspondance entre les pixels d'un panorama et les directions du ciel.
//!
//! Sur un tour complet, la largeur qui couvre 360° fixe l'échelle sans rien
//! connaître de l'objectif : `f = largeur_360 / 2π` pixels par radian. La seule
//! autre donnée nécessaire est la ligne de l'image qui correspond à l'horizon
//! (hauteur 0°).

use std::f64::consts::PI;

/// Projection du panorama.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Projection {
    /// Panorama de téléphone : la hauteur suit l'arc tangente de l'écart de ligne.
    Cylindrique,
    /// Sphère photo : la hauteur est proportionnelle à l'écart de ligne.
    Equirectangulaire,
}

impl Projection {
    /// Nom employé dans le fichier `.horizon.json`.
    pub fn nom(self) -> &'static str {
        match self {
            Self::Cylindrique => "cylindrique",
            Self::Equirectangulaire => "equirectangulaire",
        }
    }
}

/// Géométrie d'un panorama, en pixels de la photo.
#[derive(Debug, Clone, PartialEq)]
pub struct Geometrie {
    pub projection: Projection,
    /// Largeur, en pixels, qui correspond à un tour complet.
    pub largeur_360: f64,
    /// Ligne de l'image retenue comme hauteur 0°.
    pub ligne_horizon: f64,
    /// Colonne dont l'azimut vrai est connu…
    pub colonne_reference: f64,
    /// … et cet azimut, en degrés.
    pub azimut_reference: f64,
}

impl Geometrie {
    /// Pixels par radian, horizontalement comme verticalement.
    pub fn pixels_par_radian(&self) -> f64 {
        self.largeur_360 / (2.0 * PI)
    }

    /// Hauteur apparente, en degrés, de la ligne `ligne` de l'image.
    pub fn hauteur(&self, ligne: f64) -> f64 {
        let ecart = self.ligne_horizon - ligne;
        match self.projection {
            Projection::Cylindrique => (ecart / self.pixels_par_radian()).atan().to_degrees(),
            Projection::Equirectangulaire => ecart * 360.0 / self.largeur_360,
        }
    }

    /// Ligne de l'image qui correspond à la hauteur `hauteur` (degrés).
    pub fn ligne(&self, hauteur: f64) -> f64 {
        let ecart = match self.projection {
            Projection::Cylindrique => hauteur.to_radians().tan() * self.pixels_par_radian(),
            Projection::Equirectangulaire => hauteur * self.largeur_360 / 360.0,
        };
        self.ligne_horizon - ecart
    }

    /// Azimut vrai, en degrés dans [0, 360[, de la colonne `colonne`.
    ///
    /// L'azimut croît vers la droite de l'image : on tourne dans le sens des
    /// aiguilles d'une montre en balayant le panorama de gauche à droite.
    pub fn azimut(&self, colonne: f64) -> f64 {
        (self.azimut_reference + (colonne - self.colonne_reference) * 360.0 / self.largeur_360)
            .rem_euclid(360.0)
    }

    /// Première colonne, dans [0, largeur_360[, d'azimut `azimut`.
    pub fn colonne(&self, azimut: f64) -> f64 {
        let delta = (azimut - self.azimut_reference).rem_euclid(360.0);
        (self.colonne_reference + delta * self.largeur_360 / 360.0).rem_euclid(self.largeur_360)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cylindre() -> Geometrie {
        Geometrie {
            projection: Projection::Cylindrique,
            largeur_360: 3600.0,
            ligne_horizon: 500.0,
            colonne_reference: 900.0,
            azimut_reference: 0.0,
        }
    }

    #[test]
    fn hauteur_et_ligne_sont_reciproques() {
        for projection in [Projection::Cylindrique, Projection::Equirectangulaire] {
            let g = Geometrie { projection, ..cylindre() };
            for h in [-20.0, -1.5, 0.0, 3.25, 26.4, 45.0, 70.0] {
                assert!((g.hauteur(g.ligne(h)) - h).abs() < 1e-9, "{projection:?} {h}");
            }
        }
    }

    #[test]
    fn echelle_d_un_tour_complet() {
        let g = cylindre();
        // 45° au-dessus de l'horizon : tan(45°) × 3600 / 2π pixels plus haut.
        let attendu = 500.0 - 3600.0 / (2.0 * PI);
        assert!((g.ligne(45.0) - attendu).abs() < 1e-9);
        // En équirectangulaire, 10 pixels valent 1° quand 3 600 pixels font le tour.
        let e = Geometrie { projection: Projection::Equirectangulaire, ..cylindre() };
        assert!((e.hauteur(490.0) - 1.0).abs() < 1e-12);
    }

    #[test]
    fn azimut_croit_vers_la_droite_et_boucle() {
        let g = cylindre();
        assert_eq!(g.azimut(900.0), 0.0);
        assert!((g.azimut(1800.0) - 90.0).abs() < 1e-9);
        assert!((g.azimut(0.0) - 270.0).abs() < 1e-9);
        assert!((g.azimut(4500.0) - 0.0).abs() < 1e-9);
        for az in [0.0, 12.5, 90.0, 271.0, 359.9] {
            assert!((g.azimut(g.colonne(az)) - az).abs() < 1e-9);
        }
    }
}
