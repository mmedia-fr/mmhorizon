// SPDX-License-Identifier: GPL-3.0-or-later
//! Exports vers les logiciels courants : AstroMosaic (seestar_alp), N.I.N.A., Stellarium.

use crate::format::FichierHorizon;
use crate::zip_simple;

/// Hauteur prudente d'un secteur : inconnu = masqué.
fn hauteur(f: &FichierHorizon, degre: i64) -> f64 {
    f.hauteur_min_deg[degre.rem_euclid(f.hauteur_min_deg.len() as i64) as usize].unwrap_or(90.0)
}

/// Masque AstroMosaic : 71 hauteurs limites, de 0° à 350° par pas de 5°, chacune
/// la plus haute des cinq degrés qui l'entourent. Un tableau JSON, à reporter
/// dans `horizonHard`.
pub fn astromosaic(f: &FichierHorizon) -> String {
    let valeurs: Vec<String> = (0..71)
        .map(|k| {
            let centre = k * 5;
            let h = (-2..=2).map(|d| hauteur(f, centre + d)).fold(f64::MIN, f64::max);
            format!("{:.1}", h.max(0.0))
        })
        .collect();
    format!("[{}]\n", valeurs.join(", "))
}

/// Fichier d'horizon N.I.N.A. (`.hrz`) : une paire « azimut hauteur » par ligne.
pub fn nina(f: &FichierHorizon) -> String {
    let mut texte = String::new();
    for (i, h) in f.hauteur_min_deg.iter().enumerate() {
        if let Some(h) = h {
            texte.push_str(&format!("{} {:.1}\n", i as f64 * f.pas_deg, h));
        }
    }
    texte
}

/// Identifiant ASCII tiré du nom du site, pour le dossier du paysage.
pub fn identifiant(nom: &str) -> String {
    let mut id = String::new();
    for c in nom.chars() {
        let c = match c {
            'à' | 'â' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'î' | 'ï' => 'i',
            'ô' | 'ö' => 'o',
            'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            c => c.to_ascii_lowercase(),
        };
        if c.is_ascii_alphanumeric() {
            id.push(c);
        } else if !id.ends_with('_') && !id.is_empty() {
            id.push('_');
        }
    }
    let id = id.trim_end_matches('_').to_string();
    if id.is_empty() { "horizon".to_string() } else { id }
}

/// « +44d3'54.36" » : la notation des coordonnées dans `landscape.ini`.
fn dms(v: f64) -> String {
    let signe = if v < 0.0 { '-' } else { '+' };
    let a = v.abs();
    let d = a.trunc();
    let m = ((a - d) * 60.0).trunc();
    let s = (a - d - m / 60.0) * 3600.0;
    format!("{signe}{}d{}'{:.2}\"", d as i64, m as i64, s)
}

/// Paysage polygonal Stellarium, en archive ZIP installable depuis Stellarium
/// (fenêtre Ciel et vision › Paysage › Ajouter / supprimer des paysages).
pub fn stellarium(f: &FichierHorizon) -> Vec<u8> {
    let id = identifiant(&f.site.nom);
    let mut horizon = String::new();
    for (i, h) in f.hauteur_min_deg.iter().enumerate() {
        if let Some(h) = h {
            horizon.push_str(&format!("{} {:.1}\n", i as f64 * f.pas_deg, h));
        }
    }
    let nom = if f.site.nom.trim().is_empty() { "Horizon relevé" } else { f.site.nom.trim() };
    let mut ini = format!(
        "[landscape]\nname = {nom}\ntype = polygonal\nauthor = MMhorizon\n\
         description = Horizon relevé par MMhorizon{} (photo {}).\n\
         polygonal_horizon_list = horizon.txt\npolygonal_horizon_list_mode = azDeg_altDeg\n\
         polygonal_angle_rotatez = 0\nground_color = 0.15,0.17,0.12\n\
         horizon_line_color = 0.9,0.3,0.2\nminimal_brightness = 0.15\n",
        f.releve.date.as_deref().map(|d| format!(" le {}", &d[..d.len().min(10)])).unwrap_or_default(),
        f.releve.photo.nom,
    );
    if let (Some(lat), Some(lon)) = (f.site.latitude, f.site.longitude) {
        ini.push_str(&format!(
            "\n[location]\nplanet = Earth\nlatitude = {}\nlongitude = {}\naltitude = {}\n",
            dms(lat),
            dms(lon),
            f.site.altitude_m.unwrap_or(0.0).round() as i64
        ));
    }
    zip_simple::archive(&[
        (format!("{id}/landscape.ini"), ini.into_bytes()),
        (format!("{id}/horizon.txt"), horizon.into_bytes()),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifiants() {
        assert_eq!(identifiant("Plateforme du toit — Astaffort"), "plateforme_du_toit_astaffort");
        assert_eq!(identifiant("  "), "horizon");
    }

    #[test]
    fn coordonnees_stellarium() {
        assert_eq!(dms(44.0651), "+44d3'54.36\"");
        assert_eq!(dms(-0.5), "-0d30'0.00\"");
    }
}
