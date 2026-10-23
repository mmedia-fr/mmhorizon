// SPDX-License-Identifier: GPL-3.0-or-later
//! Fichier `.horizon.json`, version 1 — spécification : `docs/FORMAT.md`.

use crate::arrondi;
use crate::releve::Releve;
use serde::{Deserialize, Serialize};

pub const FORMAT: &str = "horizon";
pub const VERSION_FORMAT: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FichierHorizon {
    pub format: String,
    pub version: u32,
    pub site: Site,
    pub releve: InfosReleve,
    pub pas_deg: f64,
    pub hauteur_min_deg: Vec<Option<f64>>,
    #[serde(default)]
    pub au_dela_du_champ: Vec<u16>,
    #[serde(default)]
    pub masques_hauts: Vec<MasqueHaut>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Site {
    pub nom: String,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub altitude_m: Option<f64>,
    pub hauteur_objectif_m: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InfosReleve {
    pub date: Option<String>,
    pub logiciel: String,
    pub photo: Photo,
    pub projection: String,
    pub couverture_azimut_deg: f64,
    /// « gpano », « chevauchement » ou « supposee » (champ ajouté à la v1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub couverture_methode: Option<String>,
    pub orientation: String,
    pub objectif: Objectif,
    pub nord: Nord,
    pub ligne_horizon_px: f64,
    pub champ_vertical_deg: Champ,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Photo {
    pub nom: String,
    pub sha256: String,
    pub largeur_px: u32,
    pub hauteur_px: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Objectif {
    #[serde(rename = "type")]
    pub genre: String,
    pub focale_eq35_mm: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Nord {
    pub methode: String,
    pub colonne_px: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Champ {
    pub bas: f64,
    pub haut: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MasqueHaut {
    pub azimut_debut: f64,
    pub azimut_fin: f64,
    pub hauteur_debut_deg: f64,
    pub hauteur_fin_deg: Option<f64>,
}

impl MasqueHaut {
    /// Vrai si la direction (azimut, hauteur) tombe dans le masque. L'intervalle
    /// d'azimut se parcourt dans le sens horaire et peut passer par 0°.
    pub fn contient(&self, azimut: f64, hauteur: f64) -> bool {
        let etendue = (self.azimut_fin - self.azimut_debut).rem_euclid(360.0);
        let d = (azimut - self.azimut_debut).rem_euclid(360.0);
        d <= etendue
            && hauteur >= self.hauteur_debut_deg
            && self.hauteur_fin_deg.is_none_or(|fin| hauteur <= fin)
    }
}

impl FichierHorizon {
    /// Fichier d'un relevé, hauteurs arrondies au dixième de degré.
    pub fn depuis_releve(r: &Releve, site: Site) -> Self {
        let p = r.profil();
        let (bas, haut) = r.champ_vertical();
        Self {
            format: FORMAT.to_string(),
            version: VERSION_FORMAT,
            site,
            releve: InfosReleve {
                date: r.metadonnees.date.clone(),
                logiciel: format!("MMhorizon {}", crate::VERSION),
                photo: Photo {
                    nom: r.nom_photo.clone(),
                    sha256: r.sha256.clone(),
                    largeur_px: r.largeur,
                    hauteur_px: r.hauteur,
                },
                projection: r.geometrie.projection.nom().to_string(),
                couverture_azimut_deg: arrondi(r.couverture_deg(), 1),
                couverture_methode: Some(r.couverture.nom().to_string()),
                orientation: r.orientation().to_string(),
                objectif: Objectif {
                    genre: r.objectif().to_string(),
                    focale_eq35_mm: r.metadonnees.focale_eq35_mm,
                },
                nord: Nord {
                    methode: r.nord_methode.clone(),
                    colonne_px: r.colonne_nord().map(|c| arrondi(c, 1)),
                },
                ligne_horizon_px: arrondi(r.geometrie.ligne_horizon, 1),
                champ_vertical_deg: Champ { bas: arrondi(bas, 1), haut: arrondi(haut, 1) },
            },
            pas_deg: 1.0,
            hauteur_min_deg: p.hauteur_min.iter().map(|v| v.map(|h| arrondi(h, 1))).collect(),
            au_dela_du_champ: p.au_dela_du_champ,
            masques_hauts: Vec::new(),
        }
    }

    pub fn vers_json(&self) -> String {
        let mut texte = serde_json::to_string_pretty(self).expect("sérialisation du relevé");
        texte.push('\n');
        texte
    }

    pub fn depuis_json(texte: &str) -> Result<Self, String> {
        let f: Self = serde_json::from_str(texte)
            .map_err(|e| format!("fichier .horizon.json illisible : {e}"))?;
        if f.format != FORMAT {
            return Err(format!("format « {} » inattendu (attendu : « {FORMAT} »)", f.format));
        }
        if f.version != VERSION_FORMAT {
            return Err(format!(
                "version {} du format non prise en charge (attendue : {VERSION_FORMAT})",
                f.version
            ));
        }
        let attendu = (360.0 / f.pas_deg).round() as usize;
        if f.pas_deg <= 0.0 || f.hauteur_min_deg.len() != attendu {
            return Err(format!(
                "hauteur_min_deg compte {} valeurs, {attendu} attendues",
                f.hauteur_min_deg.len()
            ));
        }
        Ok(f)
    }

    /// Visibilité d'une direction apparente : `Some(true)` dégagée, `Some(false)`
    /// masquée, `None` incertaine (secteur non couvert, ou obstacle au-delà du
    /// champ de la photo).
    pub fn visible(&self, azimut: f64, hauteur: f64) -> Option<bool> {
        let n = self.hauteur_min_deg.len() as i64;
        let i = ((azimut / self.pas_deg).round() as i64).rem_euclid(n) as usize;
        let min = self.hauteur_min_deg[i]?;
        if hauteur < min {
            return Some(false);
        }
        if self.au_dela_du_champ.contains(&(i as u16)) {
            return None;
        }
        if self.masques_hauts.iter().any(|m| m.contient(azimut, hauteur)) {
            return Some(false);
        }
        Some(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fichier() -> FichierHorizon {
        let mut hauteurs = vec![Some(10.0); 360];
        hauteurs[45] = None;
        FichierHorizon {
            format: FORMAT.into(),
            version: 1,
            site: Site { nom: "essai".into(), ..Default::default() },
            releve: InfosReleve {
                date: None,
                logiciel: "essai".into(),
                photo: Photo { nom: "p.jpg".into(), sha256: "0".into(), largeur_px: 1, hauteur_px: 1 },
                projection: "cylindrique".into(),
                couverture_azimut_deg: 360.0,
                couverture_methode: None,
                orientation: "inconnue".into(),
                objectif: Objectif { genre: "inconnu".into(), focale_eq35_mm: None },
                nord: Nord { methode: "designation".into(), colonne_px: Some(0.0) },
                ligne_horizon_px: 0.0,
                champ_vertical_deg: Champ { bas: -26.0, haut: 26.0 },
            },
            pas_deg: 1.0,
            hauteur_min_deg: hauteurs,
            au_dela_du_champ: vec![90],
            masques_hauts: vec![MasqueHaut {
                azimut_debut: 350.0,
                azimut_fin: 10.0,
                hauteur_debut_deg: 40.0,
                hauteur_fin_deg: Some(60.0),
            }],
        }
    }

    #[test]
    fn lecture_d_une_direction() {
        let f = fichier();
        assert_eq!(f.visible(200.0, 9.9), Some(false));
        assert_eq!(f.visible(200.0, 10.0), Some(true));
        assert_eq!(f.visible(45.2, 30.0), None, "secteur non couvert");
        assert_eq!(f.visible(90.0, 30.0), None, "au-delà du champ");
        assert_eq!(f.visible(90.0, 5.0), Some(false));
        assert_eq!(f.visible(355.0, 50.0), Some(false), "masque haut passant par 0°");
        assert_eq!(f.visible(5.0, 50.0), Some(false));
        assert_eq!(f.visible(5.0, 65.0), Some(true));
        assert_eq!(f.visible(20.0, 50.0), Some(true));
        assert_eq!(f.visible(359.6, 20.0), Some(true), "359,6° tombe dans le secteur 0");
    }

    #[test]
    fn aller_retour_json() {
        let f = fichier();
        let relu = FichierHorizon::depuis_json(&f.vers_json()).unwrap();
        assert_eq!(relu, f);
        let texte = f.vers_json().replace("\"version\": 1", "\"version\": 2");
        assert!(FichierHorizon::depuis_json(&texte).unwrap_err().contains("version 2"));
    }

    #[test]
    fn champ_inconnu_ignore() {
        let texte = fichier().vers_json().replacen('{', "{\n  \"champ_futur\": 1,", 1);
        assert!(FichierHorizon::depuis_json(&texte).is_ok());
    }
}
