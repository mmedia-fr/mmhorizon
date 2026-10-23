// SPDX-License-Identifier: GPL-3.0-or-later
//! Détection de la limite ciel / obstacles, et du recouvrement d'un tour complet.
//!
//! **Limite.** Chaque pixel reçoit une probabilité d'être du ciel, par un modèle
//! appris sur la photo elle-même : le haut de l'image donne l'allure du ciel
//! (couleur, texture), le bas celle du sol. Dans chaque colonne, la limite est
//! la ligne qui sépare le mieux « ciel au-dessus » et « obstacle au-dessous » ;
//! d'une colonne à la suivante, un saut se paie, de sorte que la limite suit
//! les ruptures franches (angle d'un toit) sans s'accrocher au bruit d'une
//! colonne isolée. Le tout se résout exactement, colonne par colonne
//! (programmation dynamique, coût de saut linéaire).
//!
//! **Recouvrement.** Un panorama de téléphone dépasse souvent le tour : sa fin
//! reprend son début. Le décalage où la bande de départ se retrouve donne la
//! largeur exacte de 360°.

use image::RgbImage;

/// Coût d'un saut d'une ligne entre deux colonnes voisines, en pixels mal classés.
const PENALITE_SAUT: f32 = 0.4;

/// Écarts minimaux des modèles : un ciel parfaitement uni ne doit pas rendre
/// suspect le moindre dégradé.
const ECART_MIN: [f32; 4] = [0.06, 0.06, 0.06, 0.03];

/// Image de travail : couleurs normalisées et gradient de luminance.
pub struct ImageAnalyse {
    pub largeur: usize,
    pub hauteur: usize,
    pixels: Vec<[f32; 3]>,
    gradient: Vec<f32>,
}

impl ImageAnalyse {
    pub fn depuis(image: &RgbImage) -> Self {
        let (w, h) = (image.width() as usize, image.height() as usize);
        let pixels: Vec<[f32; 3]> = image
            .pixels()
            .map(|p| [p[0] as f32 / 255.0, p[1] as f32 / 255.0, p[2] as f32 / 255.0])
            .collect();
        let lum: Vec<f32> =
            pixels.iter().map(|p| 0.299 * p[0] + 0.587 * p[1] + 0.114 * p[2]).collect();
        let mut gradient = vec![0.0; w * h];
        for y in 0..h {
            let (ym, yp) = (y.saturating_sub(1), (y + 1).min(h - 1));
            for x in 0..w {
                let (xm, xp) = (x.saturating_sub(1), (x + 1).min(w - 1));
                let gx = lum[y * w + xp] - lum[y * w + xm];
                let gy = lum[yp * w + x] - lum[ym * w + x];
                gradient[y * w + x] = gx.abs() + gy.abs();
            }
        }
        Self { largeur: w, hauteur: h, pixels, gradient }
    }

    fn caracteristiques(&self, i: usize) -> [f32; 4] {
        let p = self.pixels[i];
        [p[0], p[1], p[2], self.gradient[i]]
    }
}

struct Modele {
    centre: [f32; 4],
    ecart: [f32; 4],
}

/// Médiane et écart absolu médian, insensibles aux quelques colonnes où un
/// obstacle déborde dans la bande d'apprentissage.
fn modele(img: &ImageAnalyse, lignes: std::ops::Range<usize>) -> Modele {
    let w = img.largeur;
    let mut valeurs: [Vec<f32>; 4] = Default::default();
    for y in lignes {
        for x in 0..w {
            let c = img.caracteristiques(y * w + x);
            for k in 0..4 {
                valeurs[k].push(c[k]);
            }
        }
    }
    let mut centre = [0.0; 4];
    let mut ecart = [0.0; 4];
    for k in 0..4 {
        let m = mediane(&mut valeurs[k]);
        let mut dev: Vec<f32> = valeurs[k].iter().map(|v| (v - m).abs()).collect();
        centre[k] = m;
        ecart[k] = (1.4826 * mediane(&mut dev)).max(ECART_MIN[k]);
    }
    Modele { centre, ecart }
}

pub(crate) fn mediane(v: &mut [f32]) -> f32 {
    if v.is_empty() {
        return 0.0;
    }
    let n = v.len() / 2;
    *v.select_nth_unstable_by(n, |a, b| a.total_cmp(b)).1
}

fn proba_ciel(c: &[f32; 4], ciel: &Modele, sol: &Modele) -> f32 {
    let mut llr = 0.0f32;
    for k in 0..4 {
        let zc = (c[k] - ciel.centre[k]) / ciel.ecart[k];
        let zs = (c[k] - sol.centre[k]) / sol.ecart[k];
        llr += 0.5 * (zs * zs - zc * zc) + (sol.ecart[k] / ciel.ecart[k]).ln();
    }
    1.0 / (1.0 + (-llr.clamp(-30.0, 30.0)).exp())
}

/// Pour chaque colonne, la première ligne qui n'est plus du ciel (0 = l'obstacle
/// touche le haut de l'image ; `hauteur` = ciel jusqu'en bas).
pub fn detecter(img: &ImageAnalyse) -> Vec<u32> {
    let (w, h) = (img.largeur, img.hauteur);
    if w == 0 || h < 8 {
        return vec![h as u32; w];
    }
    let bande_ciel = ((h as f32 * 0.03).ceil() as usize).clamp(1, h / 4);
    let debut_sol = ((h as f32 * 0.9) as usize).min(h - 1);
    let ciel = modele(img, 0..bande_ciel);
    let sol = modele(img, debut_sol..h);

    let n = h + 1;
    let mut cout = vec![0f32; n];
    let mut cumul = vec![0f32; n];
    let mut minimum = vec![0f32; n];
    let mut origine = vec![0u32; n];
    let mut retour = vec![0u32; w * n];
    let mut proba = vec![0f32; h];

    for x in 0..w {
        for (y, p) in proba.iter_mut().enumerate() {
            *p = proba_ciel(&img.caracteristiques(y * w + x), &ciel, &sol);
        }
        // Coût de la limite en y : ciel manqué au-dessus, ciel pris pour obstacle au-dessous.
        let mut au_dessus = 0.0;
        let mut au_dessous: f32 = proba.iter().sum();
        for y in 0..n {
            cout[y] = au_dessus + au_dessous;
            if y < h {
                au_dessus += 1.0 - proba[y];
                au_dessous -= proba[y];
            }
        }
        if x == 0 {
            cumul.copy_from_slice(&cout);
            continue;
        }
        // Transformée en distance L1 de la colonne précédente, avec l'origine du minimum.
        minimum.copy_from_slice(&cumul);
        for (y, o) in origine.iter_mut().enumerate() {
            *o = y as u32;
        }
        for y in 1..n {
            let c = minimum[y - 1] + PENALITE_SAUT;
            if c < minimum[y] {
                minimum[y] = c;
                origine[y] = origine[y - 1];
            }
        }
        for y in (0..n - 1).rev() {
            let c = minimum[y + 1] + PENALITE_SAUT;
            if c < minimum[y] {
                minimum[y] = c;
                origine[y] = origine[y + 1];
            }
        }
        let base = minimum.iter().copied().fold(f32::INFINITY, f32::min);
        for y in 0..n {
            cumul[y] = cout[y] + minimum[y] - base;
            retour[x * n + y] = origine[y];
        }
    }

    let mut lignes = vec![0u32; w];
    let mut y = (0..n).min_by(|&a, &b| cumul[a].total_cmp(&cumul[b])).unwrap_or(0) as u32;
    for x in (0..w).rev() {
        lignes[x] = y;
        if x > 0 {
            y = retour[x * n + y as usize];
        }
    }
    lignes
}

/// Largeur, en colonnes d'analyse, d'un tour complet — si la fin du panorama
/// reprend nettement son début. `None` si aucun recouvrement ne ressort.
pub fn largeur_tour(img: &ImageAnalyse) -> Option<usize> {
    let (w, h) = (img.largeur, img.hauteur);
    let k = (w / 24).max(8);
    if w < 4 * k || h < 8 {
        return None;
    }
    // Le ciel, uniforme, se ressemble partout : on compare la texture de la moitié basse.
    let lignes: Vec<usize> = ((h * 2 / 5)..h).step_by(2).collect();
    let g = |x: usize, y: usize| img.gradient[y * w + x];
    let ecart = |p: usize| -> f32 {
        let mut s = 0.0;
        for x in 0..k {
            for &y in &lignes {
                s += (g(x, y) - g(p + x, y)).abs();
            }
        }
        s / (k * lignes.len()) as f32
    };
    let candidats: Vec<(usize, f32)> = (w / 2..=w - k).map(|p| (p, ecart(p))).collect();
    let mut tri: Vec<f32> = candidats.iter().map(|c| c.1).collect();
    let med = mediane(&mut tri);
    let (p, e) = candidats.iter().copied().min_by(|a, b| a.1.total_cmp(&b.1))?;
    (e < 0.5 * med).then_some(p)
}
