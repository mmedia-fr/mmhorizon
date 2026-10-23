# Format `.horizon.json` — version 1

Fichier texte JSON (UTF-8) décrivant la zone de ciel observable depuis un lieu.
Il est produit par MMhorizon et se lit sans lui : tout logiciel peut le
produire ou le consommer.

## Conventions

- **Azimut** : azimut vrai (géographique), en degrés, 0 = nord, 90 = est,
  compté dans le sens horaire.
- **Hauteur** : en degrés au-dessus de l'horizon astronomique du point de
  prise de vue (plan horizontal passant par l'objectif). Une valeur négative
  est possible depuis un point élevé. Les hauteurs sont **apparentes**, telles
  que photographiées : la réfraction atmosphérique est à appliquer à la
  position calculée de la cible, pas au masque.
- **Secteur** : la valeur d'indice `i` couvre l'azimut `[i − 0,5 ; i + 0,5[`
  et retient la **plus haute** limite ciel / obstacle du secteur (lecture
  prudente).

## Structure

```json
{
  "format": "horizon",
  "version": 1,
  "site": {
    "nom": "Plateforme du toit",
    "latitude": 44.0651,
    "longitude": 0.6505,
    "altitude_m": 72.0,
    "hauteur_objectif_m": 1.15
  },
  "releve": {
    "date": "2026-10-25T14:32:00+02:00",
    "logiciel": "MMhorizon 0.1.0",
    "photo": {
      "nom": "PANO_20261025_143200.jpg",
      "sha256": "…",
      "largeur_px": 11520,
      "hauteur_px": 2160
    },
    "projection": "cylindrique",
    "couverture_azimut_deg": 360,
    "couverture_methode": "chevauchement",
    "orientation": "paysage",
    "objectif": { "type": "principal", "focale_eq35_mm": 26 },
    "nord": { "methode": "designation", "colonne_px": 4321.5 },
    "ligne_horizon_px": 1080,
    "champ_vertical_deg": { "bas": -26.4, "haut": 26.4 }
  },
  "pas_deg": 1,
  "hauteur_min_deg": [ 12.4, 12.6, 13.0 ],
  "au_dela_du_champ": [ 118, 119, 120 ],
  "masques_hauts": [
    { "azimut_debut": 200, "azimut_fin": 215,
      "hauteur_debut_deg": 48.0, "hauteur_fin_deg": null }
  ]
}
```

`hauteur_min_deg` compte toujours `360 / pas_deg` valeurs (360 en version 1) ;
l'exemple est tronqué.

## Champs

| Champ | Sens |
|---|---|
| `format`, `version` | Identifiant du format et version. Un lecteur refuse une version qu'il ne connaît pas. |
| `site.latitude`, `site.longitude` | Degrés décimaux, WGS 84, est positif ; `null` si inconnus. |
| `site.altitude_m` | Altitude du point de prise de vue, si connue. |
| `site.hauteur_objectif_m` | Hauteur de l'objectif au-dessus du sol ou de la plateforme, si renseignée. |
| `releve.date` | Date et heure de la photo, ISO 8601 avec décalage horaire. |
| `releve.photo.sha256` | Empreinte du fichier source, pour retrouver la photo d'origine. |
| `releve.projection` | `cylindrique` (panorama de téléphone) ou `equirectangulaire` (sphère photo). |
| `releve.couverture_azimut_deg` | Étendue horizontale du panorama ; 360 pour un tour complet. |
| `releve.couverture_methode` | Comment le tour a été mesuré : `gpano` (métadonnées), `chevauchement` (la fin du panorama reprend son début) ou `supposee` (la photo est tenue pour un tour exact). Facultatif. |
| `releve.orientation` | `portrait`, `paysage` ou `inconnue` — déduite de la photo. |
| `releve.objectif` | `principal`, `ultra-grand-angle` ou `inconnu`, et focale équivalente 35 mm lue dans l'EXIF. |
| `releve.nord` | Comment le nord a été fixé — `designation` (désigné sur la photo), `gpano` (cap des métadonnées) ou `non-fixe` (azimuts relatifs, fichier inutilisable pour l'observation) — et la colonne du panorama qui lui correspond. |
| `releve.ligne_horizon_px` | Ligne de l'image retenue comme hauteur 0°. |
| `releve.champ_vertical_deg` | Hauteurs extrêmes couvertes par l'image. |
| `pas_deg` | Pas d'azimut du tableau ; 1 en version 1. |
| `hauteur_min_deg` | Pour chaque secteur, hauteur au-dessus de laquelle le ciel est dégagé. `null` si le secteur n'est pas couvert par la photo. |
| `au_dela_du_champ` | Indices des secteurs où l'obstacle dépasse le haut de l'image : leur valeur vaut alors `champ_vertical_deg.haut` et n'est qu'un minimum. |
| `masques_hauts` | Zones bloquées **au-dessus** du ciel dégagé (avant-toit, branches) : de `azimut_debut` à `azimut_fin` dans le sens horaire (le passage par 0° est permis), de `hauteur_debut_deg` à `hauteur_fin_deg` (`null` : au moins jusqu'au haut du champ). |

## Lecture

Une cible de position apparente (azimut `A`, hauteur `h`) est **visible** si :

1. `h ≥ hauteur_min_deg[arrondi(A / pas_deg) mod 360]`, et
2. aucun masque haut ne contient le couple (`A`, `h`).

Un secteur à `null` ou marqué « au-delà du champ » doit être tenu pour
**incertain**, et non pour dégagé.

## Évolution

Tout changement incompatible incrémente `version`. Un lecteur ignore les
champs qu'il ne connaît pas : l'ajout d'un champ ne change pas la version.
