// SPDX-License-Identifier: GPL-3.0-or-later
//! Relevé : une photo, sa géométrie, la limite détectée, et le profil qui en découle.

use crate::detection::{self, ImageAnalyse};
use crate::geometrie::{Geometrie, Projection};
use crate::metadonnees::{self, Metadonnees};
use image::{imageops, Rgb, RgbImage};
use sha2::{Digest, Sha256};

/// Largeur de travail de la détection : 3 600 colonnes, soit 0,1° par colonne
/// sur un tour complet. Au-delà, le gain de précision est sans objet devant la
/// parallaxe du support (de l'ordre du demi-degré).
pub const LARGEUR_ANALYSE: u32 = 3600;

/// Part du champ de l'objectif qu'un panorama de téléphone garde en hauteur. Il en
/// rogne un peu pour raccorder les vues (de l'ordre du quart sur le Galaxy S26 à
/// l'ultra grand-angle), jamais la moitié ; et il ne peut pas voir plus haut que
/// l'objectif. Hors de ces bornes, un GPano décrit une autre photo que celle-ci.
const CHAMP_GARDE_MIN: f64 = 0.6;
const CHAMP_GARDE_MAX: f64 = 1.2;

/// Profil par degré d'azimut vrai.
#[derive(Debug, Clone, PartialEq)]
pub struct Profil {
    /// Hauteur au-dessus de laquelle le ciel est dégagé ; `None` hors couverture.
    pub hauteur_min: Vec<Option<f64>>,
    /// Secteurs où l'obstacle dépasse le haut de l'image.
    pub au_dela_du_champ: Vec<u16>,
}

/// Comment la largeur de 360° a été établie.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Couverture {
    /// Métadonnées GPano de la photo.
    GPano,
    /// Recouvrement détecté entre la fin et le début du panorama.
    Chevauchement,
    /// Un même point désigné aux deux bouts du panorama.
    Designation,
    /// Faute de mieux, la photo est tenue pour un tour exact.
    Supposee,
}

impl Couverture {
    pub fn nom(self) -> &'static str {
        match self {
            Self::GPano => "gpano",
            Self::Chevauchement => "chevauchement",
            Self::Designation => "designation",
            Self::Supposee => "supposee",
        }
    }
}

pub struct Releve {
    pub nom_photo: String,
    pub sha256: String,
    pub largeur: u32,
    pub hauteur: u32,
    pub metadonnees: Metadonnees,
    pub geometrie: Geometrie,
    pub couverture: Couverture,
    /// Pourquoi le GPano de la photo a été écarté, s'il l'a été.
    pub gpano_ecarte: Option<String>,
    /// « designation », « gpano » ou « non-fixe ».
    pub nord_methode: String,
    analyse: RgbImage,
    echelle_x: f64,
    echelle_y: f64,
    /// Ligne de la limite, en pixels de la photo, pour chaque colonne d'analyse.
    limite: Vec<f64>,
}

impl Releve {
    /// Ouvre une photo (JPEG ou PNG) et détecte la limite ciel / obstacles.
    pub fn depuis_octets(octets: &[u8], nom: &str) -> Result<Self, String> {
        let metadonnees = metadonnees::lire(octets);
        // Sans copie de l'image décodée : un panorama de 35 Mpx en pèse 106 Mo, ce
        // qui compte dans un navigateur ou sur un téléphone.
        let image = image::load_from_memory(octets)
            .map_err(|e| format!("photo illisible : {e}"))?
            .into_rgb8();
        let image = redresser(image, metadonnees.orientation_exif);
        let sha256 = Sha256::digest(octets).iter().map(|o| format!("{o:02x}")).collect();
        Self::depuis_image(image, nom, sha256, metadonnees)
    }

    /// Variante sur une image déjà décodée.
    pub fn depuis_image(
        image: RgbImage,
        nom: &str,
        sha256: String,
        metadonnees: Metadonnees,
    ) -> Result<Self, String> {
        let (largeur, hauteur) = image.dimensions();
        if largeur < 360 || hauteur < 32 {
            return Err(format!("photo trop petite pour un panorama ({largeur} × {hauteur})"));
        }
        let la = largeur.min(LARGEUR_ANALYSE);
        let ha = ((hauteur as f64) * la as f64 / largeur as f64).round().max(16.0) as u32;
        let analyse = if la == largeur {
            image
        } else {
            imageops::resize(&image, la, ha, imageops::FilterType::Triangle)
        };
        let echelle_x = largeur as f64 / analyse.width() as f64;
        let echelle_y = hauteur as f64 / analyse.height() as f64;
        let donnees = ImageAnalyse::depuis(&analyse);
        let (geometrie, couverture, nord_gpano, gpano_ecarte) =
            geometrie_initiale(&metadonnees, largeur, hauteur, &donnees, echelle_x);
        let limite = detection::detecter(&donnees)
            .into_iter()
            .map(|y| y as f64 * echelle_y)
            .collect();
        Ok(Self {
            nom_photo: nom.to_string(),
            sha256,
            largeur,
            hauteur,
            metadonnees,
            geometrie,
            couverture,
            gpano_ecarte,
            nord_methode: if nord_gpano { "gpano" } else { "non-fixe" }.to_string(),
            analyse,
            echelle_x,
            echelle_y,
            limite,
        })
    }

    /// Fixe l'azimut vrai `azimut` (degrés) de la colonne `colonne` de la photo.
    pub fn fixer_reference(&mut self, colonne: f64, azimut: f64) {
        self.geometrie.colonne_reference = colonne;
        self.geometrie.azimut_reference = azimut.rem_euclid(360.0);
        self.nord_methode = "designation".to_string();
    }

    /// Désigne la colonne de la photo qui pointe au nord.
    pub fn fixer_nord(&mut self, colonne: f64) {
        self.fixer_reference(colonne, 0.0);
    }

    pub fn nord_fixe(&self) -> bool {
        self.nord_methode != "non-fixe"
    }

    /// Colonne de la photo qui pointe au nord, si le nord est fixé.
    pub fn colonne_nord(&self) -> Option<f64> {
        self.nord_fixe().then(|| self.geometrie.colonne(0.0))
    }

    /// Fixe la largeur d'un tour par un même point vu aux deux bouts du
    /// panorama, aux colonnes `a` et `b` : quand la photo dépasse le tour sans que
    /// le recouvrement ait été détecté. La largeur règle l'échelle des azimuts et
    /// des hauteurs ; le nord et la ligne d'horizon restent sur leurs colonne et
    /// ligne, et sont à redésigner s'ils en dépendaient (orientation sur le soleil).
    pub fn fixer_tour(&mut self, a: f64, b: f64) -> Result<(), String> {
        let tour = (b - a).abs();
        let largeur = self.largeur as f64;
        if !(largeur / 2.0..=largeur).contains(&tour) {
            return Err(format!(
                "tour de {tour:.0} px : il faut désigner un même point au début et à la fin du \
                 panorama, soit un écart de {:.0} à {largeur:.0} px",
                largeur / 2.0
            ));
        }
        self.geometrie.largeur_360 = tour;
        self.couverture = Couverture::Designation;
        Ok(())
    }

    /// Ligne de la photo retenue comme horizon (hauteur 0°).
    pub fn fixer_ligne_horizon(&mut self, ligne: f64) {
        self.geometrie.ligne_horizon = ligne.clamp(0.0, self.hauteur as f64);
    }

    /// Pose la limite à la ligne `ligne` sur les colonnes `x0` à `x1` de la photo.
    pub fn retoucher(&mut self, x0: f64, x1: f64, ligne: f64) {
        let n = self.limite.len();
        let indice = |x: f64| ((x / self.echelle_x).floor().max(0.0) as usize).min(n - 1);
        let (a, b) = (indice(x0.min(x1)), indice(x0.max(x1)));
        let y = ligne.clamp(0.0, self.hauteur as f64);
        for v in &mut self.limite[a..=b] {
            *v = y;
        }
    }

    /// Points de la limite, en pixels de la photo : (colonne, ligne).
    pub fn limite(&self) -> Vec<(f64, f64)> {
        self.limite
            .iter()
            .enumerate()
            .map(|(i, &y)| ((i as f64 + 0.5) * self.echelle_x, y))
            .collect()
    }

    /// Image de travail de la détection, redressée, au plus 3 600 colonnes : une
    /// interface s'en sert d'aperçu. Un de ses pixels vaut
    /// `largeur / image_analyse().width()` pixels de la photo.
    pub fn image_analyse(&self) -> &RgbImage {
        &self.analyse
    }

    /// Hauteurs extrêmes couvertes par l'image (bas, haut), en degrés.
    pub fn champ_vertical(&self) -> (f64, f64) {
        (self.geometrie.hauteur(self.hauteur as f64), self.geometrie.hauteur(0.0))
    }

    /// Étendue horizontale du panorama, plafonnée à 360°.
    pub fn couverture_deg(&self) -> f64 {
        (self.largeur as f64 * 360.0 / self.geometrie.largeur_360).min(360.0)
    }

    /// Profil par degré : pour chaque secteur [i − 0,5 ; i + 0,5[, la plus haute
    /// limite rencontrée (lecture prudente).
    pub fn profil(&self) -> Profil {
        let mut hauteur_min: Vec<Option<f64>> = vec![None; 360];
        let mut hors = [false; 360];
        let marge = 0.5 * self.echelle_y;
        for (x, y) in self.limite() {
            let secteur = (self.geometrie.azimut(x).round() as usize) % 360;
            let h = self.geometrie.hauteur(y);
            hauteur_min[secteur] = Some(hauteur_min[secteur].map_or(h, |m| m.max(h)));
            if y <= marge {
                hors[secteur] = true;
            }
        }
        let au_dela_du_champ = (0..360u16).filter(|&i| hors[i as usize]).collect();
        Profil { hauteur_min, au_dela_du_champ }
    }

    /// Orientation de l'appareil à la prise de vue, déduite du champ vertical et
    /// de la focale : « portrait », « paysage » ou « inconnue ».
    pub fn orientation(&self) -> &'static str {
        if self.geometrie.projection != Projection::Cylindrique {
            return "inconnue";
        }
        let Some(f) = self.metadonnees.focale_eq35_mm else {
            return "inconnue";
        };
        let (bas, haut) = self.champ_vertical();
        let champ = haut - bas;
        let (paysage, portrait) = champs_objectif(f);
        let (nom, ecart, attendu) = if (champ - portrait).abs() < (champ - paysage).abs() {
            ("portrait", (champ - portrait).abs(), portrait)
        } else {
            ("paysage", (champ - paysage).abs(), paysage)
        };
        if ecart / attendu > 0.25 {
            "inconnue"
        } else {
            nom
        }
    }

    /// Objectif reconnu par sa focale équivalente.
    pub fn objectif(&self) -> &'static str {
        match self.metadonnees.focale_eq35_mm {
            None => "inconnu",
            Some(f) if f < 20.0 => "ultra-grand-angle",
            Some(f) if f <= 35.0 => "principal",
            Some(_) => "teleobjectif",
        }
    }

    /// Image réduite où sont tracés la limite (rouge), l'horizon (cyan) et le
    /// nord (jaune) : sert à juger la détection d'un coup d'œil.
    pub fn apercu(&self) -> RgbImage {
        let mut img = self.analyse.clone();
        let (w, h) = (img.width(), img.height());
        let ligne_h = (self.geometrie.ligne_horizon / self.echelle_y).round() as i64;
        if (0..h as i64).contains(&ligne_h) {
            for x in (0..w).step_by(2) {
                img.put_pixel(x, ligne_h as u32, Rgb([0, 230, 230]));
            }
        }
        for (i, &y) in self.limite.iter().enumerate() {
            let yy = (y / self.echelle_y).round() as i64;
            for d in -1..=1 {
                let l = yy + d;
                if (0..h as i64).contains(&l) {
                    img.put_pixel(i as u32, l as u32, Rgb([255, 30, 30]));
                }
            }
        }
        if let Some(c) = self.colonne_nord() {
            let mut x = c;
            while x < self.largeur as f64 {
                let xa = (x / self.echelle_x) as u32;
                if xa < w {
                    for y in 0..h {
                        img.put_pixel(xa, y, Rgb([255, 220, 0]));
                    }
                }
                x += self.geometrie.largeur_360;
            }
        }
        img
    }
}

/// Applique l'orientation EXIF (les rotations ; les miroirs n'arrivent pas en photo).
fn redresser(mut image: RgbImage, orientation: Option<u32>) -> RgbImage {
    match orientation {
        Some(3) => {
            imageops::rotate180_in_place(&mut image);
            image
        }
        Some(6) => imageops::rotate90(&image),
        Some(8) => imageops::rotate270(&image),
        _ => image,
    }
}

/// Champs verticaux, en degrés, d'un objectif de focale équivalente `focale` (mm)
/// tenu en paysage puis en portrait. Capteur 4:3 de même diagonale que le
/// 24 × 36 : 34,6 × 25,96 mm.
fn champs_objectif(focale: f64) -> (f64, f64) {
    (2.0 * (12.98 / focale).atan().to_degrees(), 2.0 * (17.3 / focale).atan().to_degrees())
}

/// Un GPano de panorama cylindrique fixe l'échelle, donc le champ vertical de
/// l'image : il doit rester dans ce que l'objectif de la photo peut couvrir. Rend
/// la raison de l'écarter, ou `None` s'il tient (ou si rien ne permet d'en juger).
fn incoherence_gpano(m: &Metadonnees, geo: &Geometrie, largeur: u32, hauteur: u32) -> Option<String> {
    let focale = m.focale_eq35_mm?;
    if geo.projection != Projection::Cylindrique || focale <= 0.0 {
        return None;
    }
    let champ = geo.hauteur(0.0) - geo.hauteur(hauteur as f64);
    let (paysage, portrait) = champs_objectif(focale);
    if (CHAMP_GARDE_MIN * paysage..=CHAMP_GARDE_MAX * portrait).contains(&champ) {
        return None;
    }
    let annonce = largeur as f64 * 360.0 / geo.largeur_360;
    Some(format!(
        "GPano écarté : {annonce:.0}° annoncés donnent un champ vertical de {champ:.0}°, \
         quand l'objectif ({focale:.0} mm éq.) couvre {paysage:.0}° en paysage et {portrait:.0}° en portrait"
    ))
}

/// Géométrie de départ : GPano s'il est là et cohérent avec l'objectif, sinon
/// sphère 2:1, sinon panorama cylindrique dont le tour se lit par recouvrement.
/// Rend aussi si le nord est déjà connu (cap GPano), et la raison d'avoir écarté
/// le GPano.
fn geometrie_initiale(
    m: &Metadonnees,
    largeur: u32,
    hauteur: u32,
    donnees: &ImageAnalyse,
    echelle_x: f64,
) -> (Geometrie, Couverture, bool, Option<String>) {
    let (w, h) = (largeur as f64, hauteur as f64);
    let mut geo = Geometrie {
        projection: Projection::Cylindrique,
        largeur_360: w,
        ligne_horizon: h / 2.0,
        colonne_reference: 0.0,
        azimut_reference: 0.0,
    };
    let mut ecarte = None;
    if let Some(g) = &m.gpano {
        if let (Some(complete), Some(rognee)) = (g.largeur_complete, g.largeur_rognee) {
            if complete > 0.0 && rognee > 0.0 {
                let k = w / rognee;
                let mut gp = geo.clone();
                let equi = g
                    .projection
                    .as_deref()
                    .is_some_and(|p| p.eq_ignore_ascii_case("equirectangular"));
                if equi {
                    gp.projection = Projection::Equirectangulaire;
                    if let (Some(hc), Some(haut)) = (g.hauteur_complete, g.haut_rogne) {
                        gp.ligne_horizon = (hc / 2.0 - haut) * k;
                    }
                }
                gp.largeur_360 = complete * k;
                let mut nord = false;
                if let Some(cap) = g.cap_deg {
                    gp.colonne_reference = (complete / 2.0 - g.gauche_rognee.unwrap_or(0.0)) * k;
                    gp.azimut_reference = cap.rem_euclid(360.0);
                    nord = true;
                }
                ecarte = incoherence_gpano(m, &gp, largeur, hauteur);
                if ecarte.is_none() {
                    return (gp, Couverture::GPano, nord, None);
                }
            }
        }
    }
    if largeur == 2 * hauteur {
        geo.projection = Projection::Equirectangulaire;
        return (geo, Couverture::Supposee, false, ecarte);
    }
    match detection::largeur_tour(donnees) {
        Some(p) => {
            geo.largeur_360 = p as f64 * echelle_x;
            (geo, Couverture::Chevauchement, false, ecarte)
        }
        None => (geo, Couverture::Supposee, false, ecarte),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadonnees::GPano;
    use crate::synthese;

    fn releve(image: RgbImage, focale: Option<f64>, complete: f64) -> Releve {
        let largeur = image.width() as f64;
        let m = Metadonnees {
            focale_eq35_mm: focale,
            gpano: Some(GPano {
                projection: Some("cylindrical".into()),
                largeur_complete: Some(complete),
                largeur_rognee: Some(largeur),
                gauche_rognee: Some((complete - largeur) / 2.0),
                ..GPano::default()
            }),
            ..Metadonnees::default()
        };
        Releve::depuis_image(image, "essai.png", String::new(), m).unwrap()
    }

    // Le panorama de synthèse fait un tour de 1 800 px sur 360 de haut : champ
    // vertical de 64°, celui d'un 21 mm éq. tenu en paysage (63°).

    #[test]
    fn gpano_fidele_d_un_demi_tour_conserve() {
        let demi = imageops::crop_imm(&synthese::panorama(1800, 360, 0), 0, 0, 900, 360).to_image();
        let r = releve(demi, Some(21.0), 1800.0);
        assert_eq!(r.couverture, Couverture::GPano);
        assert_eq!(r.gpano_ecarte, None);
        assert!((r.geometrie.largeur_360 - 1800.0).abs() < 1e-9);
        assert!((r.couverture_deg() - 180.0).abs() < 1e-9);
    }

    #[test]
    fn gpano_incoherent_avec_l_objectif_ecarte() {
        // Tour complet annoncé pour un demi-tour, comme le Galaxy S26 à l'ultra
        // grand-angle : le champ vertical tomberait à 33°.
        let r = releve(synthese::panorama(1800, 360, 90), Some(21.0), 3780.0);
        assert_eq!(r.couverture, Couverture::Chevauchement);
        assert!(r.gpano_ecarte.as_deref().is_some_and(|m| m.starts_with("GPano écarté : 180°")));
        assert!((r.geometrie.largeur_360 - 1800.0).abs() < 10.0, "{}", r.geometrie.largeur_360);
        assert_eq!(r.orientation(), "paysage");
    }

    #[test]
    fn redressement_a_180_degres_sur_place() {
        let image = synthese::panorama(400, 60, 0);
        assert_eq!(redresser(image.clone(), Some(3)), imageops::rotate180(&image));
        assert_eq!(redresser(image.clone(), Some(1)), image);
    }

    fn sans_metadonnees(image: RgbImage) -> Releve {
        Releve::depuis_image(image, "essai.png", String::new(), Metadonnees::default()).unwrap()
    }

    #[test]
    fn tour_designe_quand_le_recouvrement_echappe() {
        // Recouvrement de 600 px manqué : la photo est tenue pour un tour de
        // 2 400 px, azimuts et hauteurs s'en trouvent comprimés. Un même point
        // désigné aux deux bouts rend le tour vrai.
        let mut r = sans_metadonnees(synthese::panorama(1800, 360, 600));
        r.geometrie.largeur_360 = 2400.0;
        r.couverture = Couverture::Supposee;
        r.fixer_nord(0.0);
        let site = || crate::Site { nom: "essai".into(), ..Default::default() };
        let faux = crate::controle::ecarts(&crate::FichierHorizon::depuis_releve(&r, site()));
        assert!(faux.is_err() || faux.unwrap().0 > 1.0);

        r.fixer_tour(2050.0, 250.0).unwrap();
        assert_eq!(r.couverture, Couverture::Designation);
        assert_eq!(r.geometrie.largeur_360, 1800.0);
        assert_eq!(r.colonne_nord(), Some(0.0));
        let f = crate::FichierHorizon::depuis_releve(&r, site());
        assert_eq!(f.releve.couverture_methode.as_deref(), Some("designation"));
        let (moyen, p95) = crate::controle::ecarts(&f).unwrap();
        assert!(moyen < 0.3 && p95 < 0.8, "moyen {moyen:.2}, 95 % {p95:.2}");
    }

    #[test]
    fn tour_impossible_refuse_sans_rien_changer() {
        let mut r = sans_metadonnees(synthese::panorama(1800, 360, 90));
        let (geo, couverture) = (r.geometrie.clone(), r.couverture);
        let e = r.fixer_tour(0.0, 1900.0).unwrap_err();
        assert!(e.starts_with("tour de 1900 px : il faut désigner un même point"), "{e}");
        assert!(r.fixer_tour(100.0, 900.0).is_err());
        assert_eq!(r.geometrie, geo);
        assert_eq!(r.couverture, couverture);
    }

    #[test]
    fn gpano_conserve_sans_focale_pour_en_juger() {
        let r = releve(synthese::panorama(1800, 360, 90), None, 3780.0);
        assert_eq!(r.couverture, Couverture::GPano);
        assert_eq!(r.gpano_ecarte, None);
    }
}
