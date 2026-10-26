// SPDX-License-Identifier: GPL-3.0-or-later
//! MMhorizon dans une page web : le noyau de calcul compilé en WebAssembly.
//!
//! Même déroulé que l'application (`core/src/releve_qt.rs`) : la page ouvre la
//! photo, désigne le nord et l'horizon, retouche la limite, puis demande les
//! fichiers produits. Le calcul se fait dans le navigateur : la photo n'est
//! envoyée nulle part, seuls les fichiers produits partent, si la page les envoie.
//! Interface décrite dans `docs/WASM.md`.
//!
//! La logique se tient dans des fonctions qui rendent `Result<_, String>`, que les
//! tests natifs éprouvent ; les méthodes exportées ne font que traduire l'erreur
//! en `Error` JavaScript.

use mmhorizon_noyau::{controle as controle_noyau, exports, FichierHorizon, Releve as ReleveNoyau, Site};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

/// Version du noyau, inscrite dans les fichiers produits.
#[wasm_bindgen]
pub fn version() -> String {
    mmhorizon_noyau::VERSION.to_string()
}

/// Auto-contrôle de la chaîne sur un panorama de synthèse dont le profil est connu.
#[wasm_bindgen]
pub fn controle() -> Result<String, JsError> {
    controle_noyau::controle().map_err(|e| JsError::new(&e))
}

/// Site du relevé tel que la page le transmet, en JSON : tout est facultatif.
#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct SiteDemande {
    nom: Option<String>,
    latitude: Option<f64>,
    longitude: Option<f64>,
    altitude_m: Option<f64>,
    hauteur_objectif_m: Option<f64>,
}

/// Site du fichier produit : ce que la page donne, complété par l'EXIF de la photo
/// (comme `mmhorizon-calcul --nom --lat --lon --alt`). Nom par défaut : celui de
/// la photo, sans extension.
fn site(r: &ReleveNoyau, demande: Option<&str>) -> Result<Site, String> {
    let d: SiteDemande = match demande.map(str::trim).filter(|t| !t.is_empty()) {
        Some(texte) => serde_json::from_str(texte).map_err(|e| format!("site : {e}"))?,
        None => SiteDemande::default(),
    };
    Ok(Site {
        nom: d.nom.filter(|n| !n.trim().is_empty()).unwrap_or_else(|| stem(&r.nom_photo)),
        latitude: d.latitude.or(r.metadonnees.latitude),
        longitude: d.longitude.or(r.metadonnees.longitude),
        altitude_m: d.altitude_m.or(r.metadonnees.altitude_m),
        hauteur_objectif_m: d.hauteur_objectif_m,
    })
}

fn stem(nom: &str) -> String {
    std::path::Path::new(nom)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn exporter(f: &FichierHorizon, genre: &str) -> Result<Vec<u8>, String> {
    match genre {
        "astromosaic" => Ok(exports::astromosaic(f).into_bytes()),
        "nina" => Ok(exports::nina(f).into_bytes()),
        "stellarium" => Ok(exports::stellarium(f)),
        _ => Err(format!("export inconnu : « {genre} » (astromosaic, nina ou stellarium)")),
    }
}

/// Ce que l'EXIF de la photo dit du lieu et de la prise de vue.
#[derive(Serialize)]
struct InfosPhoto<'a> {
    latitude: Option<f64>,
    longitude: Option<f64>,
    altitude_m: Option<f64>,
    date: Option<&'a str>,
    focale_eq35_mm: Option<f64>,
}

/// État du relevé, pour une barre d'état ou un formulaire.
#[derive(Serialize)]
struct Infos<'a> {
    largeur: u32,
    hauteur: u32,
    projection: &'static str,
    couverture_deg: f64,
    couverture_methode: &'static str,
    tour_px: f64,
    ligne_horizon_px: f64,
    nord_methode: &'a str,
    nord_colonne_px: Option<f64>,
    champ_vertical_deg: [f64; 2],
    orientation: &'static str,
    objectif: &'static str,
    au_dela_du_champ: usize,
    gpano_ecarte: Option<&'a str>,
    photo: InfosPhoto<'a>,
}

fn infos(r: &ReleveNoyau) -> Infos<'_> {
    let (bas, haut) = r.champ_vertical();
    let m = &r.metadonnees;
    Infos {
        largeur: r.largeur,
        hauteur: r.hauteur,
        projection: r.geometrie.projection.nom(),
        couverture_deg: r.couverture_deg(),
        couverture_methode: r.couverture.nom(),
        tour_px: r.geometrie.largeur_360,
        ligne_horizon_px: r.geometrie.ligne_horizon,
        nord_methode: &r.nord_methode,
        nord_colonne_px: r.colonne_nord(),
        champ_vertical_deg: [bas, haut],
        orientation: r.orientation(),
        objectif: r.objectif(),
        au_dela_du_champ: r.profil().au_dela_du_champ.len(),
        gpano_ecarte: r.gpano_ecarte.as_deref(),
        photo: InfosPhoto {
            latitude: m.latitude,
            longitude: m.longitude,
            altitude_m: m.altitude_m,
            date: m.date.as_deref(),
            focale_eq35_mm: m.focale_eq35_mm,
        },
    }
}

/// Image de travail en RGBA, prête pour un `ImageData` de canevas.
fn rgba(r: &ReleveNoyau) -> Vec<u8> {
    let image = r.image_analyse();
    let mut octets = Vec::with_capacity(image.width() as usize * image.height() as usize * 4);
    for p in image.pixels() {
        octets.extend_from_slice(&[p[0], p[1], p[2], 255]);
    }
    octets
}

/// Un relevé : une photo ouverte, sa géométrie et la limite détectée.
///
/// Colonnes et lignes sont en pixels de la photo redressée (orientation EXIF
/// appliquée), azimuts et hauteurs en degrés.
#[wasm_bindgen]
pub struct Releve {
    r: ReleveNoyau,
}

#[wasm_bindgen]
impl Releve {
    /// Ouvre une photo (octets JPEG ou PNG) et détecte la limite ciel / obstacles.
    #[wasm_bindgen(constructor)]
    pub fn new(octets: &[u8], nom: &str) -> Result<Releve, JsError> {
        ReleveNoyau::depuis_octets(octets, nom)
            .map(|r| Releve { r })
            .map_err(|e| JsError::new(&e))
    }

    #[wasm_bindgen(getter)]
    pub fn largeur(&self) -> u32 {
        self.r.largeur
    }

    #[wasm_bindgen(getter)]
    pub fn hauteur(&self) -> u32 {
        self.r.hauteur
    }

    /// Ligne de l'horizon (0°).
    #[wasm_bindgen(getter)]
    pub fn horizon(&self) -> f64 {
        self.r.geometrie.ligne_horizon
    }

    /// Colonne du nord ; `undefined` tant qu'il n'est pas fixé.
    #[wasm_bindgen(getter)]
    pub fn nord(&self) -> Option<f64> {
        self.r.colonne_nord()
    }

    /// Largeur d'un tour complet.
    #[wasm_bindgen(getter)]
    pub fn tour(&self) -> f64 {
        self.r.geometrie.largeur_360
    }

    /// État du relevé et métadonnées de la photo, en JSON (voir `docs/WASM.md`).
    pub fn infos(&self) -> String {
        serde_json::to_string(&infos(&self.r)).expect("sérialisation des infos")
    }

    /// Désigne la colonne qui pointe au nord.
    #[wasm_bindgen(js_name = fixerNord)]
    pub fn fixer_nord(&mut self, colonne: f64) {
        self.r.fixer_nord(colonne);
    }

    /// Fixe l'azimut vrai d'une colonne : un repère connu, le soleil par exemple.
    #[wasm_bindgen(js_name = fixerReference)]
    pub fn fixer_reference(&mut self, colonne: f64, azimut: f64) {
        self.r.fixer_reference(colonne, azimut);
    }

    /// Ligne de la photo retenue comme horizon (0°).
    #[wasm_bindgen(js_name = fixerHorizon)]
    pub fn fixer_horizon(&mut self, ligne: f64) {
        self.r.fixer_ligne_horizon(ligne);
    }

    /// Pose la limite à la ligne `ligne` entre les colonnes `x0` et `x1`.
    pub fn retoucher(&mut self, x0: f64, x1: f64, ligne: f64) {
        self.r.retoucher(x0, x1, ligne);
    }

    /// Limite détectée : `[x0, y0, x1, y1, …]`, un point par colonne d'aperçu.
    pub fn limite(&self) -> Vec<f64> {
        self.r.limite().into_iter().flat_map(|(x, y)| [x, y]).collect()
    }

    /// Azimut de la colonne ; relatif à la colonne 0 tant que le nord n'est pas fixé.
    #[wasm_bindgen(js_name = azimutDeColonne)]
    pub fn azimut_de_colonne(&self, colonne: f64) -> f64 {
        self.r.geometrie.azimut(colonne)
    }

    /// Colonne d'un azimut, dans le premier tour de la photo.
    #[wasm_bindgen(js_name = colonneDAzimut)]
    pub fn colonne_d_azimut(&self, azimut: f64) -> f64 {
        self.r.geometrie.colonne(azimut)
    }

    /// Hauteur au-dessus de l'horizon de la ligne.
    #[wasm_bindgen(js_name = hauteurDeLigne)]
    pub fn hauteur_de_ligne(&self, ligne: f64) -> f64 {
        self.r.geometrie.hauteur(ligne)
    }

    /// Ligne d'une hauteur au-dessus de l'horizon.
    #[wasm_bindgen(js_name = ligneDeHauteur)]
    pub fn ligne_de_hauteur(&self, hauteur: f64) -> f64 {
        self.r.geometrie.ligne(hauteur)
    }

    /// Largeur de l'aperçu : l'image de travail, au plus 3 600 colonnes.
    #[wasm_bindgen(getter, js_name = apercuLargeur)]
    pub fn apercu_largeur(&self) -> u32 {
        self.r.image_analyse().width()
    }

    /// Hauteur de l'aperçu.
    #[wasm_bindgen(getter, js_name = apercuHauteur)]
    pub fn apercu_hauteur(&self) -> u32 {
        self.r.image_analyse().height()
    }

    /// Pixels de l'aperçu en RGBA, pour `new ImageData(new Uint8ClampedArray(…), largeur)`.
    pub fn apercu(&self) -> Vec<u8> {
        rgba(&self.r)
    }

    /// Contenu du fichier `.horizon.json`. `site` : JSON facultatif,
    /// `{"nom", "latitude", "longitude", "altitude_m", "hauteur_objectif_m"}` ; ce
    /// qui manque vient de l'EXIF.
    #[wasm_bindgen(js_name = fichierHorizon)]
    pub fn fichier_horizon(&self, site: Option<String>) -> Result<String, JsError> {
        self.fichier(site.as_deref())
            .map(|f| f.vers_json())
            .map_err(|e| JsError::new(&e))
    }

    /// Exports : `astromosaic` (JSON), `nina` (`.hrz`) ou `stellarium` (ZIP).
    pub fn exporter(&self, genre: &str, site: Option<String>) -> Result<Vec<u8>, JsError> {
        self.fichier(site.as_deref())
            .and_then(|f| exporter(&f, genre))
            .map_err(|e| JsError::new(&e))
    }
}

impl Releve {
    fn fichier(&self, demande: Option<&str>) -> Result<FichierHorizon, String> {
        Ok(FichierHorizon::depuis_releve(&self.r, site(&self.r, demande)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::ImageFormat;
    use mmhorizon_noyau::synthese;
    use std::io::Cursor;

    fn releve() -> ReleveNoyau {
        let mut png = Vec::new();
        synthese::panorama(1800, 360, 90)
            .write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
            .unwrap();
        ReleveNoyau::depuis_octets(&png, "Panorama été.png").unwrap()
    }

    #[test]
    fn site_complete_par_l_exif_et_nomme_d_apres_la_photo() {
        let mut r = releve();
        r.metadonnees.latitude = Some(44.06);
        r.metadonnees.altitude_m = Some(129.0);
        let s = site(&r, None).unwrap();
        assert_eq!(s.nom, "Panorama été");
        assert_eq!(s.latitude, Some(44.06));
        assert_eq!(s.altitude_m, Some(129.0));
        assert_eq!(site(&r, Some("  ")).unwrap(), s);

        let s = site(&r, Some(r#"{"nom": "Toit", "latitude": 45.5, "hauteur_objectif_m": 1.2}"#)).unwrap();
        assert_eq!(s.nom, "Toit");
        assert_eq!(s.latitude, Some(45.5));
        assert_eq!(s.altitude_m, Some(129.0));
        assert_eq!(s.hauteur_objectif_m, Some(1.2));
    }

    #[test]
    fn site_refuse_un_champ_inconnu() {
        let e = site(&releve(), Some(r#"{"lat": 44.06}"#)).unwrap_err();
        assert!(e.starts_with("site : unknown field `lat`"), "{e}");
        assert!(site(&releve(), Some("{")).is_err());
    }

    #[test]
    fn exports_connus_et_inconnu() {
        let mut r = releve();
        r.fixer_nord(0.0);
        let f = FichierHorizon::depuis_releve(&r, site(&r, None).unwrap());
        assert_eq!(exporter(&f, "nina").unwrap(), exports::nina(&f).into_bytes());
        assert_eq!(exporter(&f, "astromosaic").unwrap(), exports::astromosaic(&f).into_bytes());
        assert!(exporter(&f, "stellarium").unwrap().starts_with(b"PK"));
        assert!(exporter(&f, "hrz").unwrap_err().starts_with("export inconnu : « hrz »"));
    }

    #[test]
    fn apercu_rgba_de_l_image_de_travail() {
        let r = releve();
        let (w, h) = r.image_analyse().dimensions();
        let octets = rgba(&r);
        assert_eq!(octets.len(), (w * h * 4) as usize);
        let p = r.image_analyse().get_pixel(7, 5);
        let i = ((5 * w + 7) * 4) as usize;
        assert_eq!(&octets[i..i + 4], &[p[0], p[1], p[2], 255]);
    }

    #[test]
    fn infos_suivent_le_nord() {
        let mut r = releve();
        let avant: serde_json::Value = serde_json::to_value(infos(&r)).unwrap();
        assert_eq!(avant["nord_methode"], "non-fixe");
        assert!(avant["nord_colonne_px"].is_null());
        assert_eq!(avant["largeur"], 1890);
        r.fixer_nord(450.0);
        let apres: serde_json::Value = serde_json::to_value(infos(&r)).unwrap();
        assert_eq!(apres["nord_methode"], "designation");
        assert_eq!(apres["nord_colonne_px"], 450.0);
    }
}
