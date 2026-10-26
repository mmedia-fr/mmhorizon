// SPDX-License-Identifier: GPL-3.0-or-later
//! mmhorizon-calcul — d'un panorama 360° au fichier `.horizon.json`, sans interface.

use mmhorizon_noyau::{controle, exports, synthese, FichierHorizon, Releve, Site};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const AIDE: &str = "\
mmhorizon-calcul — zone de ciel observable à partir d'un panorama 360°

Usage :
  mmhorizon-calcul PHOTO --nord X [options]
  mmhorizon-calcul --controle

Repérage :
  --nord X              colonne de la photo (pixels) qui pointe au nord vrai
  --repere X:AZ         colonne X d'azimut vrai AZ (degrés) — un repère connu
                        (sans --nord ni --repere, le cap GPano de la photo sert
                        s'il existe ; sinon les azimuts restent relatifs)
  --ligne-horizon Y     ligne de la photo à la hauteur 0° (défaut : milieu)
  --soleil X:Y          centre du soleil sur la photo : nord et ligne d'horizon
                        en découlent, d'après la date et le lieu de la prise de
                        vue (EXIF, ou --date et --lat --lon) ; exclut --nord,
                        --repere et --ligne-horizon
  --date ISO            date et heure de la prise de vue avec décalage
                        (2026-10-23T15:59:17+02:00), si l'EXIF ne la donne pas
  --tour A:B            colonnes d'un même point vu au début (A) et à la fin (B)
                        du panorama : leur écart est un tour, quand la photo
                        dépasse le tour sans que le recouvrement soit détecté

Site (si absent de l'EXIF) :
  --nom TEXTE  --lat DEG  --lon DEG  --alt M  --hauteur-objectif M

Sorties :
  --sortie FICHIER      défaut : PHOTO.horizon.json
  --exports DOSSIER     écrit aussi astromosaic.json, horizon.hrz et le paysage
                        Stellarium (ZIP)
  --apercu FICHIER.png  image réduite où sont tracés limite, horizon et nord

  --controle            auto-contrôle sur un panorama de synthèse
  --synthese FICHIER    écrit un panorama de synthèse à profil connu (PNG ou JPEG)
";

struct Options {
    photo: Option<PathBuf>,
    nord: Option<(f64, f64)>,
    ligne_horizon: Option<f64>,
    tour: Option<(f64, f64)>,
    soleil: Option<(f64, f64)>,
    date: Option<String>,
    site: Site,
    sortie: Option<PathBuf>,
    exports: Option<PathBuf>,
    apercu: Option<PathBuf>,
    controle: bool,
    synthese: Option<PathBuf>,
}

fn nombre(nom: &str, v: Option<String>) -> Result<f64, String> {
    let v = v.ok_or_else(|| format!("{nom} attend une valeur"))?;
    v.replace(',', ".").parse::<f64>().map_err(|_| format!("{nom} : « {v} » n'est pas un nombre"))
}

fn lire_options() -> Result<Options, String> {
    let mut o = Options {
        photo: None,
        nord: None,
        ligne_horizon: None,
        tour: None,
        soleil: None,
        date: None,
        site: Site::default(),
        sortie: None,
        exports: None,
        apercu: None,
        controle: false,
        synthese: None,
    };
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "-h" | "--aide" | "--help" => {
                print!("{AIDE}");
                std::process::exit(0);
            }
            "--controle" => o.controle = true,
            "--synthese" => o.synthese = Some(args.next().ok_or("--synthese attend un fichier")?.into()),
            "--nord" => o.nord = Some((nombre(&a, args.next())?, 0.0)),
            "--repere" => {
                let v = args.next().ok_or("--repere attend X:AZ")?;
                let (x, az) = v.split_once(':').ok_or("--repere attend X:AZ")?;
                o.nord = Some((nombre("--repere", Some(x.into()))?, nombre("--repere", Some(az.into()))?));
            }
            "--ligne-horizon" => o.ligne_horizon = Some(nombre(&a, args.next())?),
            "--soleil" => {
                let v = args.next().ok_or("--soleil attend X:Y")?;
                let (x, y) = v.split_once(':').ok_or("--soleil attend X:Y")?;
                o.soleil = Some((nombre("--soleil", Some(x.into()))?, nombre("--soleil", Some(y.into()))?));
            }
            "--date" => o.date = Some(args.next().ok_or("--date attend une date ISO 8601")?),
            "--tour" => {
                let v = args.next().ok_or("--tour attend A:B")?;
                let (x, y) = v.split_once(':').ok_or("--tour attend A:B")?;
                o.tour = Some((nombre("--tour", Some(x.into()))?, nombre("--tour", Some(y.into()))?));
            }
            "--nom" => o.site.nom = args.next().ok_or("--nom attend un texte")?,
            "--lat" => o.site.latitude = Some(nombre(&a, args.next())?),
            "--lon" => o.site.longitude = Some(nombre(&a, args.next())?),
            "--alt" => o.site.altitude_m = Some(nombre(&a, args.next())?),
            "--hauteur-objectif" => o.site.hauteur_objectif_m = Some(nombre(&a, args.next())?),
            "--sortie" => o.sortie = Some(args.next().ok_or("--sortie attend un fichier")?.into()),
            "--exports" => o.exports = Some(args.next().ok_or("--exports attend un dossier")?.into()),
            "--apercu" => o.apercu = Some(args.next().ok_or("--apercu attend un fichier")?.into()),
            _ if a.starts_with('-') => return Err(format!("option inconnue : {a}")),
            _ if o.photo.is_none() => o.photo = Some(a.into()),
            _ => return Err(format!("argument en trop : {a}")),
        }
    }
    if o.soleil.is_some() && (o.nord.is_some() || o.ligne_horizon.is_some()) {
        return Err("--soleil fixe déjà le nord et la ligne d'horizon : sans --nord, --repere ni --ligne-horizon".into());
    }
    Ok(o)
}

fn ecrire(chemin: &Path, contenu: &[u8]) -> Result<(), String> {
    std::fs::write(chemin, contenu).map_err(|e| format!("{} : {e}", chemin.display()))?;
    println!("écrit : {}", chemin.display());
    Ok(())
}

fn executer() -> Result<(), String> {
    let o = lire_options()?;
    if o.controle {
        println!("{}", controle::controle()?);
        return Ok(());
    }
    if let Some(chemin) = o.synthese {
        synthese::panorama(1800, 360, 90).save(&chemin).map_err(|e| format!("{} : {e}", chemin.display()))?;
        println!("écrit : {} (tour de 1 800 px prolongé de 90 px)", chemin.display());
        return Ok(());
    }
    let photo = o.photo.ok_or_else(|| AIDE.to_string())?;
    let octets = std::fs::read(&photo).map_err(|e| format!("{} : {e}", photo.display()))?;
    let nom = photo.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let mut r = Releve::depuis_octets(&octets, &nom)?;
    if let Some((a, b)) = o.tour {
        r.fixer_tour(a, b)?;
    }
    if let Some((x, y)) = o.soleil {
        let lieu = o.site.latitude.zip(o.site.longitude);
        let (azimut, hauteur) = r.soleil(o.date.as_deref(), lieu)?;
        r.orienter_sur_soleil(x, y, (azimut, hauteur));
        println!(
            "soleil à l'azimut {azimut:.2}°, hauteur {hauteur:.2}° : nord en colonne {:.0}, horizon en ligne {:.0}",
            r.colonne_nord().unwrap_or_default(),
            r.geometrie.ligne_horizon
        );
    }
    if let Some(y) = o.ligne_horizon {
        r.fixer_ligne_horizon(y);
    }
    if let Some((x, az)) = o.nord {
        r.fixer_reference(x, az);
    }
    let mut site = o.site;
    site.latitude = site.latitude.or(r.metadonnees.latitude);
    site.longitude = site.longitude.or(r.metadonnees.longitude);
    site.altitude_m = site.altitude_m.or(r.metadonnees.altitude_m);
    if site.nom.is_empty() {
        site.nom = Path::new(&nom).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    }

    let f = FichierHorizon::depuis_releve(&r, site);
    let (bas, haut) = (f.releve.champ_vertical_deg.bas, f.releve.champ_vertical_deg.haut);
    println!(
        "{} × {} px, {} ({}), tour {:.0} px, couverture {:.1}°, champ {bas:+.1}° à {haut:+.1}°",
        r.largeur,
        r.hauteur,
        f.releve.projection,
        f.releve.couverture_methode.as_deref().unwrap_or("?"),
        r.geometrie.largeur_360,
        f.releve.couverture_azimut_deg,
    );
    println!(
        "orientation {}, objectif {}, nord : {}",
        f.releve.orientation, f.releve.objectif.genre, f.releve.nord.methode
    );
    if let Some(motif) = &r.gpano_ecarte {
        eprintln!("attention : {motif}");
    }
    if !r.nord_fixe() {
        eprintln!("attention : nord non fixé — les azimuts sont relatifs à la colonne 0 (--nord X)");
    }
    if !f.au_dela_du_champ.is_empty() {
        println!("{} degrés d'azimut où l'obstacle dépasse le haut de l'image", f.au_dela_du_champ.len());
    }

    let sortie = o.sortie.unwrap_or_else(|| {
        let mut p = photo.clone();
        p.set_extension("horizon.json");
        p
    });
    ecrire(&sortie, f.vers_json().as_bytes())?;
    if let Some(dossier) = o.exports {
        std::fs::create_dir_all(&dossier).map_err(|e| format!("{} : {e}", dossier.display()))?;
        ecrire(&dossier.join("astromosaic.json"), exports::astromosaic(&f).as_bytes())?;
        ecrire(&dossier.join("horizon.hrz"), exports::nina(&f).as_bytes())?;
        let zip = format!("stellarium-{}.zip", exports::identifiant(&f.site.nom));
        ecrire(&dossier.join(zip), &exports::stellarium(&f))?;
    }
    if let Some(apercu) = o.apercu {
        r.apercu().save(&apercu).map_err(|e| format!("{} : {e}", apercu.display()))?;
        println!("écrit : {}", apercu.display());
    }
    Ok(())
}

fn main() -> ExitCode {
    match executer() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
