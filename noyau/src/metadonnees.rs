// SPDX-License-Identifier: GPL-3.0-or-later
//! Métadonnées de la photo : EXIF (lieu, date, focale, orientation) et XMP GPano.
//!
//! Tout est facultatif : une donnée absente est demandée à l'utilisateur, ou
//! inscrite comme inconnue dans le fichier produit — jamais devinée.

use std::io::Cursor;

/// Géométrie d'un panorama selon l'espace de noms XMP `GPano` (sphères photo).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GPano {
    pub projection: Option<String>,
    pub largeur_complete: Option<f64>,
    pub hauteur_complete: Option<f64>,
    pub largeur_rognee: Option<f64>,
    pub hauteur_rognee: Option<f64>,
    pub gauche_rognee: Option<f64>,
    pub haut_rogne: Option<f64>,
    /// Cap, en degrés depuis le nord, du centre du panorama complet.
    pub cap_deg: Option<f64>,
}

/// Ce que la photo dit d'elle-même.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Metadonnees {
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub altitude_m: Option<f64>,
    /// Date de prise de vue, ISO 8601, avec décalage horaire s'il est connu.
    pub date: Option<String>,
    pub focale_eq35_mm: Option<f64>,
    /// Valeur EXIF `Orientation` (1 = normale, 3, 6, 8 = rotations).
    pub orientation_exif: Option<u32>,
    pub gpano: Option<GPano>,
}

/// Lit les métadonnées d'une photo JPEG ou PNG.
pub fn lire(octets: &[u8]) -> Metadonnees {
    let mut m = Metadonnees::default();
    if let Ok(exif) = exif::Reader::new().read_from_container(&mut Cursor::new(octets)) {
        lire_exif(&exif, &mut m);
    }
    m.gpano = lire_gpano(octets);
    m
}

fn lire_exif(exif: &exif::Exif, m: &mut Metadonnees) {
    use exif::{In, Tag, Value};

    let champ = |tag: Tag| exif.get_field(tag, In::PRIMARY);
    let texte = |tag: Tag| -> Option<String> {
        match &champ(tag)?.value {
            Value::Ascii(v) => v
                .first()
                .map(|o| String::from_utf8_lossy(o).trim_matches(char::from(0)).trim().to_string())
                .filter(|s| !s.is_empty()),
            _ => None,
        }
    };
    let rationnels = |tag: Tag| -> Option<Vec<f64>> {
        match &champ(tag)?.value {
            Value::Rational(v) if !v.is_empty() => Some(v.iter().map(|r| r.to_f64()).collect()),
            _ => None,
        }
    };
    let entier = |tag: Tag| champ(tag).and_then(|c| c.value.get_uint(0));

    let degres = |v: Vec<f64>| -> Option<f64> {
        let d = v.first().copied()? + v.get(1).copied().unwrap_or(0.0) / 60.0
            + v.get(2).copied().unwrap_or(0.0) / 3600.0;
        d.is_finite().then_some(d)
    };

    if let Some(lat) = rationnels(Tag::GPSLatitude).and_then(degres) {
        let sud = texte(Tag::GPSLatitudeRef).is_some_and(|r| r.starts_with('S'));
        m.latitude = Some(if sud { -lat } else { lat });
    }
    if let Some(lon) = rationnels(Tag::GPSLongitude).and_then(degres) {
        let ouest = texte(Tag::GPSLongitudeRef).is_some_and(|r| r.starts_with('W'));
        m.longitude = Some(if ouest { -lon } else { lon });
    }
    if let Some(alt) = rationnels(Tag::GPSAltitude).and_then(|v| v.first().copied()) {
        let dessous = entier(Tag::GPSAltitudeRef) == Some(1);
        if alt.is_finite() {
            m.altitude_m = Some(if dessous { -alt } else { alt });
        }
    }
    if let Some(date) = texte(Tag::DateTimeOriginal).or_else(|| texte(Tag::DateTime)) {
        m.date = date_iso(&date, texte(Tag::OffsetTimeOriginal).as_deref());
    }
    m.focale_eq35_mm = entier(Tag::FocalLengthIn35mmFilm).filter(|&f| f > 0).map(f64::from);
    m.orientation_exif = entier(Tag::Orientation);
}

/// « 2026:10:25 14:32:00 » et « +02:00 » → « 2026-10-25T14:32:00+02:00 ».
pub fn date_iso(exif: &str, decalage: Option<&str>) -> Option<String> {
    let (jour, heure) = exif.trim().split_once(' ')?;
    let jour = jour.replace(':', "-");
    if jour.len() != 10 || heure.len() < 8 {
        return None;
    }
    let mut iso = format!("{jour}T{}", &heure[..8]);
    if let Some(d) = decalage.map(str::trim).filter(|d| d.len() == 6) {
        iso.push_str(d);
    }
    Some(iso)
}

fn lire_gpano(octets: &[u8]) -> Option<GPano> {
    // Le paquet XMP est en tête de fichier ; quelques mégaoctets suffisent
    // largement, y compris pour un XMP étendu.
    let fin = octets.len().min(4 << 20);
    let texte = String::from_utf8_lossy(&octets[..fin]);
    if !texte.contains("GPano:") {
        return None;
    }
    let nombre = |nom: &str| valeur_xmp(&texte, nom).and_then(|v| v.trim().parse::<f64>().ok());
    let g = GPano {
        projection: valeur_xmp(&texte, "ProjectionType"),
        largeur_complete: nombre("FullPanoWidthPixels"),
        hauteur_complete: nombre("FullPanoHeightPixels"),
        largeur_rognee: nombre("CroppedAreaImageWidthPixels"),
        hauteur_rognee: nombre("CroppedAreaImageHeightPixels"),
        gauche_rognee: nombre("CroppedAreaLeftPixels"),
        haut_rogne: nombre("CroppedAreaTopPixels"),
        cap_deg: nombre("PoseHeadingDegrees"),
    };
    (g != GPano::default()).then_some(g)
}

/// Valeur d'une propriété `GPano:<nom>`, écrite en attribut ou en élément.
pub fn valeur_xmp(texte: &str, nom: &str) -> Option<String> {
    let attribut = format!("GPano:{nom}=");
    if let Some(i) = texte.find(&attribut) {
        let reste = &texte[i + attribut.len()..];
        if let Some(guillemet) = reste.chars().next().filter(|c| *c == '"' || *c == '\'') {
            let corps = &reste[1..];
            if let Some(fin) = corps.find(guillemet) {
                return Some(corps[..fin].trim().to_string());
            }
        }
    }
    let ouvrante = format!("<GPano:{nom}>");
    if let Some(i) = texte.find(&ouvrante) {
        let corps = &texte[i + ouvrante.len()..];
        if let Some(fin) = corps.find('<') {
            return Some(corps[..fin].trim().to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gpano_en_attribut_et_en_element() {
        let xmp = r#"<rdf:Description GPano:ProjectionType="equirectangular"
            GPano:FullPanoWidthPixels='8192' GPano:CroppedAreaTopPixels="512">
            <GPano:PoseHeadingDegrees> 271.5 </GPano:PoseHeadingDegrees>"#;
        assert_eq!(valeur_xmp(xmp, "ProjectionType").as_deref(), Some("equirectangular"));
        assert_eq!(valeur_xmp(xmp, "FullPanoWidthPixels").as_deref(), Some("8192"));
        assert_eq!(valeur_xmp(xmp, "CroppedAreaTopPixels").as_deref(), Some("512"));
        assert_eq!(valeur_xmp(xmp, "PoseHeadingDegrees").as_deref(), Some("271.5"));
        assert_eq!(valeur_xmp(xmp, "FullPanoHeightPixels"), None);
        let g = lire_gpano(xmp.as_bytes()).unwrap();
        assert_eq!(g.largeur_complete, Some(8192.0));
        assert_eq!(g.cap_deg, Some(271.5));
    }

    #[test]
    fn date_exif_vers_iso() {
        assert_eq!(
            date_iso("2026:10:25 14:32:07", Some("+02:00")).as_deref(),
            Some("2026-10-25T14:32:07+02:00")
        );
        assert_eq!(date_iso("2026:10:25 14:32:07", None).as_deref(), Some("2026-10-25T14:32:07"));
        assert_eq!(date_iso("n'importe quoi", None), None);
    }

    #[test]
    fn photo_sans_metadonnees() {
        let m = lire(b"pas une photo");
        assert_eq!(m, Metadonnees::default());
    }
}
