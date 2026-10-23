// SPDX-License-Identifier: GPL-3.0-or-later
//! Auto-contrôle de la chaîne complète sur un panorama de synthèse.
//!
//! Sert au contrôle de fabrication de l'application (`--smoke`) et à l'outil en
//! ligne de commande (`--controle`) : décodage, recouvrement, détection,
//! géométrie, fichier, relecture et exports.

use crate::exports;
use crate::format::{FichierHorizon, Site};
use crate::releve::{Couverture, Releve};
use crate::synthese;

/// Écarts entre le profil relevé et le profil vrai : (moyen, 95e centile), en degrés.
pub fn ecarts(f: &FichierHorizon) -> Result<(f64, f64), String> {
    let mut e: Vec<f64> = Vec::with_capacity(360);
    for (i, h) in f.hauteur_min_deg.iter().enumerate() {
        let h = h.ok_or_else(|| format!("secteur {i} non couvert"))?;
        e.push((h - synthese::secteur_vrai(i)).abs());
    }
    let moyen = e.iter().sum::<f64>() / e.len() as f64;
    e.sort_by(f64::total_cmp);
    Ok((moyen, e[(e.len() * 95) / 100]))
}

/// Contrôle complet ; rend un compte rendu d'une ligne, ou la raison de l'échec.
pub fn controle() -> Result<String, String> {
    let (tour, hauteur, recouvrement) = (1800, 360, 90);
    let image = synthese::panorama(tour, hauteur, recouvrement);
    let mut png = Vec::new();
    image
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .map_err(|e| format!("encodage PNG : {e}"))?;
    let mut r = Releve::depuis_octets(&png, "synthese.png")?;
    if r.couverture != Couverture::Chevauchement {
        return Err(format!("recouvrement non détecté ({})", r.couverture.nom()));
    }
    let ecart_tour = (r.geometrie.largeur_360 - tour as f64).abs();
    if ecart_tour > 3.0 {
        return Err(format!("tour mesuré à {:.1} px au lieu de {tour}", r.geometrie.largeur_360));
    }
    r.fixer_nord(0.0);
    let f = FichierHorizon::depuis_releve(&r, Site { nom: "Synthèse".into(), ..Default::default() });
    let (moyen, p95) = ecarts(&f)?;
    if moyen > 0.3 || p95 > 0.8 {
        return Err(format!("profil trop éloigné du vrai : moyen {moyen:.2}°, 95 % {p95:.2}°"));
    }
    let relu = FichierHorizon::depuis_json(&f.vers_json())?;
    if relu != f {
        return Err("le fichier relu diffère du fichier écrit".into());
    }
    if exports::astromosaic(&f).split(',').count() != 71 {
        return Err("export AstroMosaic : 71 valeurs attendues".into());
    }
    if exports::nina(&f).lines().count() != 360 || exports::stellarium(&f).len() < 200 {
        return Err("exports N.I.N.A. ou Stellarium incomplets".into());
    }
    Ok(format!(
        "contrôle : tour {:.1} px (vrai {tour}), écart moyen {moyen:.2}°, 95 % sous {p95:.2}°",
        r.geometrie.largeur_360
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chaine_complete_sur_synthese() {
        let compte_rendu = controle().unwrap();
        println!("{compte_rendu}");
    }

    fn releve_de(image: image::RgbImage, format: image::ImageFormat) -> Releve {
        let mut octets = Vec::new();
        let dyn_image = image::DynamicImage::ImageRgb8(image);
        dyn_image.write_to(&mut std::io::Cursor::new(&mut octets), format).unwrap();
        Releve::depuis_octets(&octets, "essai").unwrap()
    }

    #[test]
    fn tour_exact_sans_recouvrement() {
        let mut r = releve_de(synthese::panorama(1800, 360, 0), image::ImageFormat::Png);
        assert_eq!(r.couverture, Couverture::Supposee);
        assert_eq!(r.geometrie.largeur_360, 1800.0);
        r.fixer_nord(0.0);
        let f = FichierHorizon::depuis_releve(&r, Site::default());
        let (moyen, p95) = ecarts(&f).unwrap();
        assert!(moyen < 0.3 && p95 < 0.8, "moyen {moyen:.2}, 95 % {p95:.2}");
    }

    #[test]
    fn photo_jpeg_compressee() {
        let mut octets = Vec::new();
        let img = synthese::panorama(1800, 360, 120);
        let mut codeur = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut octets, 70);
        codeur.encode_image(&img).unwrap();
        let mut r = Releve::depuis_octets(&octets, "essai.jpg").unwrap();
        assert_eq!(r.couverture, Couverture::Chevauchement);
        assert!((r.geometrie.largeur_360 - 1800.0).abs() <= 3.0, "{}", r.geometrie.largeur_360);
        r.fixer_nord(0.0);
        let f = FichierHorizon::depuis_releve(&r, Site::default());
        let (moyen, p95) = ecarts(&f).unwrap();
        println!("JPEG 70 : moyen {moyen:.2}°, 95 % {p95:.2}°");
        assert!(moyen < 0.3 && p95 < 0.8, "moyen {moyen:.2}, 95 % {p95:.2}");
    }

    #[test]
    fn nord_decale_et_retouche() {
        let mut r = releve_de(synthese::panorama(1800, 360, 0), image::ImageFormat::Png);
        // Le nord désigné à la colonne 450 : la colonne 0 devient l'ouest (270°).
        r.fixer_nord(450.0);
        assert!((r.geometrie.azimut(0.0) - 270.0).abs() < 1e-9);
        let avant = r.profil().hauteur_min[90].unwrap();
        // Retouche : limite remontée au haut de l'image sur le secteur de l'est.
        let x = r.geometrie.colonne(90.0);
        r.retoucher(x - 1.0, x + 1.0, 0.0);
        let p = r.profil();
        assert!(p.hauteur_min[90].unwrap() > avant + 10.0);
        assert!(p.au_dela_du_champ.contains(&90));
    }
}
