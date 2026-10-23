// SPDX-License-Identifier: GPL-3.0-or-later
//! Panorama de synthèse à profil connu : sert aux tests et au contrôle de
//! fabrication, faute de photo réelle dans l'intégration continue.

use crate::geometrie::{Geometrie, Projection};
use image::{Rgb, RgbImage};

/// Profil vrai du panorama de synthèse : collines, un bâtiment, un arbre, un poteau.
pub fn profil_vrai(azimut: f64) -> f64 {
    let a = azimut.rem_euclid(360.0);
    let r = a.to_radians();
    let mut h = 4.0 + 3.0 * (3.0 * r).sin() + 1.5 * (7.0 * r).cos();
    if (100.0..130.0).contains(&a) {
        h = h.max(21.0);
    }
    if (200.0..230.0).contains(&a) {
        let c = (a - 215.0) / 6.0;
        h = h.max(6.0 + 17.0 * (-c * c).exp());
    }
    if (300.0..302.0).contains(&a) {
        h = h.max(14.0);
    }
    h
}

/// Plus haute valeur du profil vrai sur le secteur [i − 0,5 ; i + 0,5[.
pub fn secteur_vrai(i: usize) -> f64 {
    (0..20).map(|k| profil_vrai(i as f64 - 0.5 + k as f64 / 20.0)).fold(f64::MIN, f64::max)
}

/// Géométrie du panorama de synthèse : tour de `largeur_360` pixels, horizon au milieu.
pub fn geometrie(largeur_360: u32, hauteur: u32) -> Geometrie {
    Geometrie {
        projection: Projection::Cylindrique,
        largeur_360: largeur_360 as f64,
        ligne_horizon: hauteur as f64 / 2.0,
        colonne_reference: 0.0,
        azimut_reference: 0.0,
    }
}

fn bruit(x: u32, y: u32) -> f32 {
    let mut v = x.wrapping_mul(0x9E37_79B1) ^ y.wrapping_mul(0x85EB_CA77) ^ 0x5bd1_e995;
    v ^= v >> 15;
    v = v.wrapping_mul(0x2C1B_3C6D);
    v ^= v >> 12;
    v = v.wrapping_mul(0x297A_2D39);
    v ^= v >> 15;
    v as f32 / u32::MAX as f32
}

/// Panorama cylindrique d'un tour de `largeur_360` pixels, prolongé de `recouvrement`
/// colonnes qui reprennent le début — comme le fait un téléphone qui dépasse le tour.
pub fn panorama(largeur_360: u32, hauteur: u32, recouvrement: u32) -> RgbImage {
    let geo = geometrie(largeur_360, hauteur);
    RgbImage::from_fn(largeur_360 + recouvrement, hauteur, |x, y| {
        let xs = x % largeur_360;
        let limite = geo.ligne(profil_vrai(geo.azimut(xs as f64 + 0.5)));
        if (y as f64 + 0.5) < limite {
            let t = y as f32 / hauteur as f32;
            let n = (bruit(xs, y) - 0.5) * 0.02;
            let v = |c: f32| ((c + n).clamp(0.0, 1.0) * 255.0) as u8;
            Rgb([v(0.30 + 0.35 * t), v(0.50 + 0.25 * t), v(0.92 - 0.05 * t)])
        } else {
            let v = |c: f32, k: u32| {
                let n = (bruit(xs.wrapping_add(k * 7919), y) - 0.5) * 0.25;
                ((c + n).clamp(0.0, 1.0) * 255.0) as u8
            };
            Rgb([v(0.28, 1), v(0.25, 2), v(0.14, 3)])
        }
    })
}
