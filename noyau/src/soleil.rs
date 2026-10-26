// SPDX-License-Identifier: GPL-3.0-or-later
//! Position du soleil, pour orienter un panorama sur lui.
//!
//! Formules de faible précision de l'Astronomical Almanac, justes au centième
//! de degré entre 1950 et 2050 — bien assez pour un clic sur une photo.
//! Réfraction de Saemundsson, pression et température normales : la photo
//! montre le soleil à sa hauteur apparente, relevé de 0,04° à 25° de hauteur
//! et d'un demi-degré à l'horizon.

/// Azimut (depuis le nord, vers l'est) et hauteur apparente du soleil, en
/// degrés, au lieu (`latitude`, `longitude`, degrés, est positif) et au jour
/// julien `jj` (UTC).
pub fn position(latitude: f64, longitude: f64, jj: f64) -> (f64, f64) {
    let (azimut, hauteur) = position_geometrique(latitude, longitude, jj);
    (azimut, hauteur + refraction(hauteur))
}

/// Comme [`position`], sans la réfraction.
pub fn position_geometrique(latitude: f64, longitude: f64, jj: f64) -> (f64, f64) {
    let jours = jj - 2_451_545.0;
    let longitude_moyenne = 280.460 + 0.985_647_4 * jours;
    let anomalie = (357.528 + 0.985_600_3 * jours).to_radians();
    let ecliptique =
        (longitude_moyenne + 1.915 * anomalie.sin() + 0.020 * (2.0 * anomalie).sin()).to_radians();
    let obliquite = (23.439 - 0.000_000_4 * jours).to_radians();
    let ascension = (obliquite.cos() * ecliptique.sin()).atan2(ecliptique.cos());
    let declinaison = (obliquite.sin() * ecliptique.sin()).asin();

    let siderale = 280.460_618_37 + 360.985_647_366_29 * jours + longitude;
    let angle_horaire = (siderale - ascension.to_degrees()).to_radians();
    let lat = latitude.to_radians();
    let hauteur = (declinaison.sin() * lat.sin()
        + declinaison.cos() * lat.cos() * angle_horaire.cos())
    .asin();
    let azimut = (-declinaison.cos() * angle_horaire.sin())
        .atan2(declinaison.sin() * lat.cos() - declinaison.cos() * lat.sin() * angle_horaire.cos());
    (azimut.to_degrees().rem_euclid(360.0), hauteur.to_degrees())
}

/// Réfraction atmosphérique, en degrés, d'un astre à la hauteur géométrique
/// `hauteur` ; nulle sous −1°.
pub fn refraction(hauteur: f64) -> f64 {
    if hauteur < -1.0 {
        return 0.0;
    }
    1.02 / (hauteur + 10.3 / (hauteur + 5.11)).to_radians().tan() / 60.0
}

/// Jour julien (UTC) d'une date ISO 8601 avec son décalage horaire :
/// `2026-10-23T15:59:17+02:00`, `…+0200` ou `…Z`. Sans décalage, l'heure ne
/// dit pas l'instant : refusée.
pub fn jour_julien(iso: &str) -> Result<f64, String> {
    let invalide = || format!("date « {iso} » : attendu AAAA-MM-JJTHH:MM:SS suivi du décalage (+02:00 ou Z)");
    let iso = iso.trim();
    let (date, reste) = iso.split_once('T').ok_or_else(invalide)?;
    let (heure, decalage_min) = if let Some(h) = reste.strip_suffix('Z') {
        (h, 0)
    } else {
        let i = reste.rfind(['+', '-']).ok_or_else(|| {
            format!("date « {iso} » sans décalage horaire : l'heure de la photo ne dit pas l'instant")
        })?;
        let (h, d) = reste.split_at(i);
        let signe = if d.starts_with('-') { -1 } else { 1 };
        let chiffres: String = d[1..].chars().filter(|c| *c != ':').collect();
        if chiffres.len() != 4 {
            return Err(invalide());
        }
        let hh: i64 = chiffres[..2].parse().map_err(|_| invalide())?;
        let mm: i64 = chiffres[2..].parse().map_err(|_| invalide())?;
        (h, signe * (hh * 60 + mm))
    };
    let nombres = |texte: &str, separateur: char| -> Option<Vec<f64>> {
        texte.split(separateur).map(|v| v.parse::<f64>().ok()).collect()
    };
    let d = nombres(date, '-').filter(|v| v.len() == 3).ok_or_else(invalide)?;
    let h = nombres(heure, ':').filter(|v| v.len() == 3).ok_or_else(invalide)?;
    let (annee, mois, jour) = (d[0] as i64, d[1] as i64, d[2] as i64);
    if !(1..=12).contains(&mois) || !(1..=31).contains(&jour) || h[0] >= 24.0 || h[1] >= 60.0 || h[2] >= 61.0 {
        return Err(invalide());
    }
    let secondes = h[0] * 3600.0 + h[1] * 60.0 + h[2] - decalage_min as f64 * 60.0;
    Ok(jours_depuis_1970(annee, mois, jour) as f64 + 2_440_587.5 + secondes / 86_400.0)
}

/// Jours entre le 1er janvier 1970 et la date civile (calendrier grégorien).
fn jours_depuis_1970(annee: i64, mois: i64, jour: i64) -> i64 {
    let a = if mois <= 2 { annee - 1 } else { annee };
    let ere = a.div_euclid(400);
    let annee_ere = a - ere * 400;
    let jour_annee = (153 * (mois + if mois > 2 { -3 } else { 9 }) + 2) / 5 + jour - 1;
    let jour_ere = annee_ere * 365 + annee_ere / 4 - annee_ere / 100 + jour_annee;
    ere * 146_097 + jour_ere - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

    // Astaffort, lieu du premier panorama réel.
    const LAT: f64 = 44.06504;
    const LON: f64 = 0.65041;

    #[test]
    fn jour_julien_des_dates_iso() {
        assert_eq!(jour_julien("2000-01-01T12:00:00Z").unwrap(), 2_451_545.0);
        let a = jour_julien("2026-10-23T15:59:17+02:00").unwrap();
        assert_eq!(a, jour_julien("2026-10-23T13:59:17Z").unwrap());
        assert_eq!(a, jour_julien("2026-10-23T15:59:17+0200").unwrap());
        assert_eq!(jour_julien("2026-10-23T08:59:17-05:00").unwrap(), a);
        assert!((a - 2_461_337.082_835_6).abs() < 1e-6, "{a}");
        assert!(jour_julien("2026-10-23T15:59:17").unwrap_err().contains("sans décalage horaire"));
        assert!(jour_julien("23/10/2026 15:59").is_err());
        assert!(jour_julien("2026-13-23T15:59:17Z").is_err());
    }

    #[test]
    fn position_geometrique_comme_astropy() {
        // astropy (get_sun, AltAz), même lieu : azimut, hauteur.
        for (iso, az, h) in [
            ("2026-10-23T13:59:17Z", 218.093, 26.223),
            ("2026-06-21T12:00:00Z", 180.511, 69.371),
            ("2026-12-21T08:30:00Z", 133.684, 7.765),
            ("2027-03-20T17:45:00Z", 266.511, 3.532),
        ] {
            let (a, b) = position_geometrique(LAT, LON, jour_julien(iso).unwrap());
            let ecart_az = ((a - az + 180.0).rem_euclid(360.0) - 180.0) * b.to_radians().cos();
            assert!(ecart_az.abs() < 0.03 && (b - h).abs() < 0.03, "{iso} : {a:.3} {b:.3}");
        }
    }

    #[test]
    fn soleil_apparent_du_premier_panorama() {
        // Valeurs de seestar_alp (/horizon/sun) pour la photo du 2026-10-23.
        let (az, h) = position(LAT, LON, jour_julien("2026-10-23T15:59:17+02:00").unwrap());
        assert!((az - 218.098).abs() < 0.002 && (h - 26.261).abs() < 0.002, "{az} {h}");
        assert!((refraction(25.0) - 0.035).abs() < 0.003);
        assert!((refraction(0.0) - 0.48).abs() < 0.01);
        assert_eq!(refraction(-2.0), 0.0);
    }
}
