# MMhorizon dans une page web — module WebAssembly

Le noyau de calcul compilé en WebAssembly : une page web fait le relevé dans le
navigateur, avec le calcul de l'application et de `mmhorizon-calcul`. Sur une
même photo, les fichiers produits sont identiques octet pour octet. La photo
reste dans le navigateur ; seuls les fichiers produits en sortent, si la page
les envoie.

Premier usage visé : seestar_alp, pour y charger le panorama du lieu
d'observation et en tirer l'horizon local du planificateur.

## Construire

```sh
rustup target add wasm32-unknown-unknown
# Même version que la caisse wasm-bindgen de wasm/Cargo.toml.
cargo install wasm-bindgen-cli --version 0.2.129 --locked
cargo build --manifest-path wasm/Cargo.toml --release --locked --target wasm32-unknown-unknown
wasm-bindgen --target web --out-dir dist/wasm \
    wasm/target/wasm32-unknown-unknown/release/mmhorizon_wasm.wasm
```

`dist/wasm/` contient le module ES `mmhorizon_wasm.js`, le binaire
`mmhorizon_wasm_bg.wasm` (environ 800 Ko) et leurs déclarations TypeScript.
L'intégration continue le construit et le dépose sous le nom
`mmhorizon-noyau-wasm`.

## Usage

```js
import init, { Releve } from "./mmhorizon_wasm.js";

await init(); // charge mmhorizon_wasm_bg.wasm, posé à côté du module
const octets = new Uint8Array(await fichier.arrayBuffer()); // <input type="file">
const r = new Releve(octets, fichier.name); // détection de la limite
r.fixerNord(colonne); // ou r.fixerReference(colonneDuSoleil, azimutDuSoleil)
const hrz = new TextDecoder().decode(r.exporter("nina", JSON.stringify({ nom: "Toit" })));
r.free();
```

Le calcul occupe le fil qui l'exécute pendant plusieurs secondes sur une vraie
photo : le placer dans un Web Worker garde la page réactive.

## Interface

Colonnes et lignes sont en pixels de la photo **redressée** (orientation EXIF
appliquée) ; l'aperçu rendu par `apercu()` est dans ce repère, à l'échelle
`largeur / apercuLargeur`. Azimuts et hauteurs sont en degrés. Les erreurs
arrivent en exceptions `Error`, avec un message en français.

| Membre | Rôle |
|---|---|
| `version()` | version du noyau |
| `controle()` | auto-contrôle sur un panorama de synthèse : compte rendu, ou exception |
| `new Releve(octets, nom)` | ouvre une photo JPEG ou PNG et détecte la limite ciel / obstacles |
| `largeur`, `hauteur` | dimensions de la photo |
| `horizon` | ligne de l'horizon (0°), par défaut le milieu de la photo |
| `nord` | colonne du nord ; `undefined` tant qu'il n'est pas fixé |
| `tour` | largeur d'un tour complet |
| `infos()` | état du relevé et métadonnées de la photo, en JSON (ci-dessous) |
| `fixerNord(colonne)` | désigne la colonne qui pointe au nord |
| `fixerReference(colonne, azimut)` | azimut vrai connu d'une colonne : un repère, le soleil |
| `fixerTour(a, b)` | largeur d'un tour : un même point vu au début (colonne `a`) et à la fin (`b`) du panorama, quand la photo dépasse le tour sans que le recouvrement ait été détecté ; exception si l'écart sort de [largeur / 2 ; largeur] |
| `fixerHorizon(ligne)` | ligne de l'horizon |
| `retoucher(x0, x1, ligne)` | pose la limite à `ligne` entre les colonnes `x0` et `x1` |
| `limite()` | `Float64Array` `[x0, y0, x1, y1, …]`, un point par colonne d'aperçu |
| `azimutDeColonne(c)`, `colonneDAzimut(az)` | géométrie horizontale ; azimuts relatifs à la colonne 0 tant que le nord n'est pas fixé |
| `hauteurDeLigne(l)`, `ligneDeHauteur(h)` | géométrie verticale |
| `apercuLargeur`, `apercuHauteur`, `apercu()` | image de travail (au plus 3 600 colonnes) en RGBA, pour un `ImageData` |
| `fichierHorizon(site?)` | contenu du fichier `.horizon.json` ([FORMAT.md](FORMAT.md)) |
| `exporter(genre, site?)` | `Uint8Array` : `astromosaic` (JSON), `nina` (`.hrz`) ou `stellarium` (ZIP) |
| `free()` | libère le relevé |

`site` est un JSON facultatif :
`{"nom", "latitude", "longitude", "altitude_m", "hauteur_objectif_m"}`. Ce qui
manque est lu dans l'EXIF de la photo ; le nom par défaut est celui de la photo,
sans extension — comme `mmhorizon-calcul --nom --lat --lon --alt`. Un champ
inconnu est refusé.

`infos()`, sur le panorama réel qui a servi d'épreuve — il fait 435°, le soleil
y paraît deux fois et son tour a été désigné ainsi —, nord et horizon fixés :

```json
{
  "largeur": 12672,
  "hauteur": 2800,
  "projection": "cylindrique",
  "couverture_deg": 360,
  "couverture_methode": "designation",
  "tour_px": 10467,
  "ligne_horizon_px": 1254,
  "nord_methode": "designation",
  "nord_colonne_px": 5836,
  "champ_vertical_deg": [-42.86257761734126, 36.9708851431029],
  "orientation": "paysage",
  "objectif": "ultra-grand-angle",
  "au_dela_du_champ": 4,
  "gpano_ecarte": "GPano écarté : 172° annoncés donnent un champ vertical de 37°, quand l'objectif (13 mm éq.) couvre 90° en paysage et 106° en portrait",
  "photo": {
    "latitude": 44.06504199972222,
    "longitude": 0.6504145997222223,
    "altitude_m": 129,
    "date": "2026-10-23T15:59:17+02:00",
    "focale_eq35_mm": 13
  }
}
```

`couverture_methode` vaut `gpano`, `chevauchement`, `designation` ou
`supposee` ([FORMAT.md](FORMAT.md)) ; `nord_methode` vaut `designation`, `gpano`
ou `non-fixe` ; `au_dela_du_champ`
compte les degrés d'azimut où l'obstacle dépasse le haut de l'image ;
`gpano_ecarte` est `null` quand le GPano de la photo a été retenu ou qu'il n'y
en a pas.

## Ressources

Mesuré sur ce panorama (12 672 × 2 800 px, JPEG de 10 Mo), sous Node 20 — le
moteur V8 de Chrome — sur un processeur x86-64 : ouverture et détection en
7,3 s, mémoire du module 281 Mo. `mmhorizon-calcul` natif, sur la même
machine : 4,0 s, 276 Mo. La mémoire d'un module WebAssembly ne redescend pas
après usage : recharger le module, ou terminer le Worker, la rend.

## Épreuve

`wasm/essai/essai.mjs` charge le module sous Node et vérifie que, sur une même
photo et avec les mêmes options, le `.horizon.json` et les trois exports sont
identiques octet pour octet à ceux de `mmhorizon-calcul` ; il éprouve aussi
l'aperçu, la géométrie et les erreurs. L'intégration continue le passe sur le
panorama de synthèse.
