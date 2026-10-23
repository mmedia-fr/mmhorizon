// SPDX-License-Identifier: GPL-3.0-or-later
//! Relevé en cours, exposé à QML : chargement de la photo, nord, horizon,
//! retouche de la limite, fichiers produits.
//!
//! La lecture et l'écriture des fichiers restent côté Qt (`cpp/fichiers.h`) : sous
//! Android, une photo choisie dans la galerie arrive en URL `content://`, que
//! seul Qt sait ouvrir. Le noyau reçoit et rend des octets.

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qbytearray.h");
        type QByteArray = cxx_qt_lib::QByteArray;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        /// Vrai dès qu'une photo est chargée.
        #[qproperty(bool, charge)]
        #[qproperty(i32, largeur)]
        #[qproperty(i32, hauteur)]
        /// Ligne de l'horizon (0°), en pixels de la photo.
        #[qproperty(f64, horizon)]
        /// Colonne du nord, en pixels de la photo ; négative tant qu'il n'est pas fixé.
        #[qproperty(f64, nord)]
        /// Largeur d'un tour complet, en pixels de la photo.
        #[qproperty(f64, tour)]
        /// Compte rendu d'une ligne pour la barre d'état.
        #[qproperty(QString, resume)]
        /// Dernier message d'erreur ; vide s'il n'y en a pas eu.
        #[qproperty(QString, erreur)]
        /// Incrémenté à chaque changement de la limite, pour que QML redessine.
        #[qproperty(i32, revision)]
        type Releve = super::ReleveRust;

        /// Ouvre une photo (octets JPEG ou PNG) et détecte la limite.
        #[qinvokable]
        #[cxx_name = "charger"]
        fn charger(self: Pin<&mut Releve>, octets: &QByteArray, nom: &QString) -> bool;

        /// Désigne la colonne de la photo qui pointe au nord.
        #[qinvokable]
        #[cxx_name = "fixerNord"]
        fn fixer_nord(self: Pin<&mut Releve>, colonne: f64);

        /// Ligne de la photo retenue comme horizon (0°).
        #[qinvokable]
        #[cxx_name = "fixerHorizon"]
        fn fixer_horizon(self: Pin<&mut Releve>, ligne: f64);

        /// Pose la limite à la ligne `ligne` entre les colonnes `x0` et `x1`.
        #[qinvokable]
        #[cxx_name = "retoucher"]
        fn retoucher(self: Pin<&mut Releve>, x0: f64, x1: f64, ligne: f64);

        /// Limite en pixels de la photo, en JSON : [[x, y], …].
        #[qinvokable]
        #[cxx_name = "limite"]
        fn limite(&self) -> QString;

        /// Hauteur en degrés de la ligne `ligne`, et azimut de la colonne `colonne`.
        #[qinvokable]
        #[cxx_name = "direction"]
        fn direction(&self, colonne: f64, ligne: f64) -> QString;

        /// Nom de site proposé : le nom de la photo, sans extension.
        #[qinvokable]
        #[cxx_name = "nomSite"]
        fn nom_site(&self) -> QString;

        /// Contenu du fichier `.horizon.json`.
        #[qinvokable]
        #[cxx_name = "fichierHorizon"]
        fn fichier_horizon(&self, nom_site: &QString) -> QByteArray;

        /// Exports : `astromosaic`, `nina` ou `stellarium`.
        #[qinvokable]
        #[cxx_name = "exporter"]
        fn exporter(&self, genre: &QString, nom_site: &QString) -> QByteArray;

        /// Auto-contrôle de la chaîne sur un panorama de synthèse ; « ok : … » ou la raison.
        #[qinvokable]
        #[cxx_name = "controle"]
        fn controle(&self) -> QString;
    }
}

use core::pin::Pin;
use cxx_qt::CxxQtType;
use cxx_qt_lib::{QByteArray, QString};
use mmhorizon_noyau::{controle, exports, FichierHorizon, Releve, Site};

pub struct ReleveRust {
    charge: bool,
    largeur: i32,
    hauteur: i32,
    horizon: f64,
    nord: f64,
    tour: f64,
    resume: QString,
    erreur: QString,
    revision: i32,
    releve: Option<Releve>,
}

impl Default for ReleveRust {
    fn default() -> Self {
        Self {
            charge: false,
            largeur: 0,
            hauteur: 0,
            horizon: 0.0,
            nord: -1.0,
            tour: 0.0,
            resume: QString::from("Ouvrez un panorama 360°."),
            erreur: QString::default(),
            revision: 0,
            releve: None,
        }
    }
}

impl ReleveRust {
    fn fichier(&self, nom_site: &QString) -> Option<FichierHorizon> {
        let r = self.releve.as_ref()?;
        let nom = nom_site.to_string();
        let site = Site {
            nom: if nom.trim().is_empty() { stem(&r.nom_photo) } else { nom },
            latitude: r.metadonnees.latitude,
            longitude: r.metadonnees.longitude,
            altitude_m: r.metadonnees.altitude_m,
            hauteur_objectif_m: None,
        };
        Some(FichierHorizon::depuis_releve(r, site))
    }
}

fn stem(nom: &str) -> String {
    std::path::Path::new(nom)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn resume(r: &Releve) -> String {
    let (bas, haut) = r.champ_vertical();
    let nord = if r.nord_fixe() { "nord fixé" } else { "nord à désigner" };
    let hors = r.profil().au_dela_du_champ.len();
    let mut texte = format!(
        "{} × {} px · {} · couverture {:.0}° ({}) · champ {bas:+.1}° à {haut:+.1}° · {} · {nord}",
        r.largeur,
        r.hauteur,
        r.geometrie.projection.nom(),
        r.couverture_deg(),
        r.couverture.nom(),
        r.orientation(),
    );
    if hors > 0 {
        texte.push_str(&format!(" · {hors}° au-delà du champ"));
    }
    if r.gpano_ecarte.is_some() {
        texte.push_str(" · GPano écarté");
    }
    texte
}

impl qobject::Releve {
    fn actualiser(mut self: Pin<&mut Self>) {
        let Some(r) = self.rust().releve.as_ref() else { return };
        let (horizon, nord, tour, texte) =
            (r.geometrie.ligne_horizon, r.colonne_nord().unwrap_or(-1.0), r.geometrie.largeur_360, resume(r));
        self.as_mut().set_horizon(horizon);
        self.as_mut().set_nord(nord);
        self.as_mut().set_tour(tour);
        self.as_mut().set_resume(QString::from(&texte));
        let n = *self.revision() + 1;
        self.as_mut().set_revision(n);
    }

    pub fn charger(mut self: Pin<&mut Self>, octets: &QByteArray, nom: &QString) -> bool {
        match Releve::depuis_octets(octets.as_slice(), &nom.to_string()) {
            Ok(r) => {
                let (largeur, hauteur) = (r.largeur as i32, r.hauteur as i32);
                self.as_mut().rust_mut().releve = Some(r);
                self.as_mut().set_largeur(largeur);
                self.as_mut().set_hauteur(hauteur);
                self.as_mut().set_erreur(QString::default());
                self.as_mut().set_charge(true);
                self.as_mut().actualiser();
                true
            }
            Err(e) => {
                self.as_mut().set_erreur(QString::from(&e));
                false
            }
        }
    }

    pub fn fixer_nord(mut self: Pin<&mut Self>, colonne: f64) {
        if let Some(r) = self.as_mut().rust_mut().releve.as_mut() {
            r.fixer_nord(colonne);
        }
        self.as_mut().actualiser();
    }

    pub fn fixer_horizon(mut self: Pin<&mut Self>, ligne: f64) {
        if let Some(r) = self.as_mut().rust_mut().releve.as_mut() {
            r.fixer_ligne_horizon(ligne);
        }
        self.as_mut().actualiser();
    }

    pub fn retoucher(mut self: Pin<&mut Self>, x0: f64, x1: f64, ligne: f64) {
        if let Some(r) = self.as_mut().rust_mut().releve.as_mut() {
            r.retoucher(x0, x1, ligne);
        }
        self.as_mut().actualiser();
    }

    pub fn limite(&self) -> QString {
        let points: Vec<[f64; 2]> = self
            .rust()
            .releve
            .as_ref()
            .map(|r| r.limite().into_iter().map(|(x, y)| [x.round(), y.round()]).collect())
            .unwrap_or_default();
        QString::from(&serde_json::to_string(&points).unwrap_or_else(|_| "[]".into()))
    }

    pub fn direction(&self, colonne: f64, ligne: f64) -> QString {
        let Some(r) = self.rust().releve.as_ref() else { return QString::default() };
        let az = r.geometrie.azimut(colonne);
        let h = r.geometrie.hauteur(ligne);
        let texte = if r.nord_fixe() {
            format!("azimut {az:.1}° · hauteur {h:+.1}°")
        } else {
            format!("hauteur {h:+.1}° (azimut après désignation du nord)")
        };
        QString::from(&texte)
    }

    pub fn nom_site(&self) -> QString {
        QString::from(&self.rust().releve.as_ref().map(|r| stem(&r.nom_photo)).unwrap_or_default())
    }

    pub fn fichier_horizon(&self, nom_site: &QString) -> QByteArray {
        match self.rust().fichier(nom_site) {
            Some(f) => QByteArray::from(f.vers_json().as_bytes()),
            None => QByteArray::default(),
        }
    }

    pub fn exporter(&self, genre: &QString, nom_site: &QString) -> QByteArray {
        let Some(f) = self.rust().fichier(nom_site) else { return QByteArray::default() };
        let octets: Vec<u8> = match genre.to_string().as_str() {
            "astromosaic" => exports::astromosaic(&f).into_bytes(),
            "nina" => exports::nina(&f).into_bytes(),
            "stellarium" => exports::stellarium(&f),
            _ => Vec::new(),
        };
        QByteArray::from(octets.as_slice())
    }

    pub fn controle(&self) -> QString {
        QString::from(&match controle::controle() {
            Ok(compte_rendu) => format!("ok : {compte_rendu}"),
            Err(e) => e,
        })
    }
}
