# MMhorizon

Relevé de la zone de ciel observable depuis un lieu, à partir d'une photo
panoramique 360°. Pour Windows, Linux et Android. Logiciel libre sous
**GNU GPL v3**.

> **État** : en développement. Le noyau de calcul et l'outil en ligne de
> commande `mmhorizon-calcul` fonctionnent ; l'application est un premier
> squelette.

---

## Principe

1. Prendre un panorama 360° du lieu d'observation : téléphone sur pied, en
   portrait ou en paysage, tour complet.
2. Le charger dans MMhorizon. Le lieu, l'altitude et la date sont lus dans
   les métadonnées de la photo ; on les saisit s'ils manquent.
3. Si le panorama dépasse le tour (le soleil, ou un autre repère, paraît aux
   deux bouts), toucher ce repère aux deux bouts : leur écart fait un tour.
4. Toucher le centre du soleil, si la photo le montre : sa position à
   l'heure et au lieu de la prise de vue donne le nord et la ligne
   d'horizon. Sinon, toucher le point qui est au nord, puis la ligne
   d'horizon. Chaque geste laisse sur le panorama un repère de la couleur
   de son bouton ; les boutons suivent cet ordre : Tour, Soleil, Nord,
   Horizon.
5. Vérifier la limite ciel / obstacles détectée, et la corriger au besoin au
   doigt ou à la souris.
6. Exporter.

L'orientation de la prise de vue et l'objectif utilisé se déduisent de la
photo : il n'y a rien à régler.

## Ce que l'on obtient

Un fichier `.horizon.json` qui donne, pour chaque degré d'azimut, la hauteur
au-dessus de laquelle le ciel est dégagé, ainsi que les zones masquées par le
haut (avant-toit, branches). Le format est ouvert et documenté :
[docs/FORMAT.md](docs/FORMAT.md). Tout logiciel, ou tout assistant, peut s'en
servir pour savoir si une cible est visible à une heure donnée.

Exports vers les logiciels courants :

- **AstroMosaic** (planificateur de seestar_alp) — 71 valeurs, une tous les 5° ;
- **N.I.N.A.** — fichier d'horizon `.hrz` ;
- **Stellarium** — paysage polygonal.

## Précision de la prise de vue

Sur un tour complet, la largeur de l'image vaut 360° : l'échelle s'en déduit
sans connaître l'objectif. Le seul repère supplémentaire est la ligne
d'horizon. Un panorama de téléphone dépasse souvent le tour ; quand la
reprise du début n'est pas détectée, un même repère touché aux deux bouts
donne la largeur du tour — sans elle, azimuts et hauteurs seraient faussés.

Le champ vertical dépend de l'orientation et de l'objectif — environ ±26° en
paysage avec l'objectif principal, ±34° en portrait, ±45° en paysage avec
l'ultra grand-angle. Un obstacle qui dépasse le haut de l'image est marqué
« au-delà du champ » : on sait qu'il masque le ciel au moins jusque-là.

L'objectif d'un téléphone sur pied est décalé de quelques centimètres de
l'axe de rotation. L'erreur qui en résulte est de l'ordre de 0,7° pour un
obstacle à 5 m et de 0,2° à 20 m ; elle n'est pas corrigée.

## Outil en ligne de commande

`mmhorizon-calcul` fait le même calcul sans interface, sur tout système :

```
mmhorizon-calcul panorama.jpg --nord 4321 --exports exports --apercu apercu.png
```

- `--nord X` : colonne de la photo, en pixels, qui pointe au nord vrai ;
  `--repere X:AZ` pour un repère d'azimut connu ;
- `--ligne-horizon Y` : ligne de la photo à 0° (par défaut, le milieu) ;
- `--soleil X:Y` : centre du soleil sur la photo ; le nord et la ligne
  d'horizon s'en déduisent, d'après la date et le lieu de la prise de vue
  (EXIF, ou `--date` et `--lat --lon`) ;
- `--tour A:B` : colonnes d'un même point vu au début et à la fin du
  panorama, quand la photo dépasse le tour sans que le recouvrement soit
  détecté (le soleil qui apparaît deux fois, par exemple) ;
- `--nom`, `--lat`, `--lon`, `--alt` : le site, si l'EXIF ne le donne pas ;
- `--exports DOSSIER` : AstroMosaic, N.I.N.A. et Stellarium en plus du
  `.horizon.json` ;
- `--apercu FICHIER.png` : image réduite où sont tracés la limite détectée
  (rouge), l'horizon (cyan) et le nord (jaune).

`mmhorizon-calcul --controle` éprouve toute la chaîne sur un panorama de
synthèse dont le profil est connu.

## Dans une page web

Le noyau existe aussi en module WebAssembly : une page web fait le relevé dans
le navigateur, avec le même calcul — mêmes fichiers, octet pour octet. La photo
ne quitte pas le navigateur. Interface et construction :
[docs/WASM.md](docs/WASM.md).

## Construire

- Noyau et outil : `cargo build --release --manifest-path noyau/Cargo.toml`.
- Module WebAssembly : voir [docs/WASM.md](docs/WASM.md).
- Application : Qt 6.4 ou plus récent, Rust stable, CMake ;
  `cmake -S . -B _build -DCMAKE_BUILD_TYPE=Release && cmake --build _build`.
  L'intégration continue (`.github/workflows/socle.yml`) construit et éprouve
  Windows, Linux (AppImage), macOS et Android.

## Licence

GNU General Public License v3 — voir [LICENSE](LICENSE).
