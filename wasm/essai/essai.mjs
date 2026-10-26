// SPDX-License-Identifier: GPL-3.0-or-later
// Épreuve du module WebAssembly sous Node : sur une même photo, il doit produire
// octet pour octet les fichiers de mmhorizon-calcul.
//
//   node wasm/essai/essai.mjs MODULE PHOTO REFERENCE.horizon.json DOSSIER_EXPORTS \
//        [--nord X] [--ligne-horizon Y] [--nom TEXTE]
//
// MODULE : dossier écrit par `wasm-bindgen --target web` ; la référence et le
// dossier d'exports viennent de `mmhorizon-calcul PHOTO … --sortie … --exports …`
// lancé avec les mêmes options.

import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import { basename, join } from "node:path";
import { pathToFileURL } from "node:url";

const [module, photo, reference, dossier, ...reste] = process.argv.slice(2);
if (!dossier) {
  console.error("usage : essai.mjs MODULE PHOTO REFERENCE.horizon.json DOSSIER_EXPORTS [--nord X] [--ligne-horizon Y] [--nom TEXTE]");
  process.exit(2);
}
const options = {};
for (let i = 0; i < reste.length; i += 2) options[reste[i]] = reste[i + 1];

const mm = await import(pathToFileURL(join(module, "mmhorizon_wasm.js")));
const sortie = mm.initSync({ module: readFileSync(join(module, "mmhorizon_wasm_bg.wasm")) });
console.log(`noyau ${mm.version()} : ${mm.controle()}`);

const debut = performance.now();
const r = new mm.Releve(readFileSync(photo), basename(photo));
const duree = performance.now() - debut;
if (options["--ligne-horizon"] !== undefined) r.fixerHorizon(Number(options["--ligne-horizon"]));
assert.equal(r.nord, undefined);
if (options["--nord"] !== undefined) r.fixerNord(Number(options["--nord"]));
const site = options["--nom"] !== undefined ? JSON.stringify({ nom: options["--nom"] }) : undefined;

// Les fichiers produits, comparés à ceux de l'outil natif.
assert.equal(r.fichierHorizon(site), readFileSync(reference, "utf8"), ".horizon.json");
const zip = readdirSync(dossier).find((n) => n.startsWith("stellarium-") && n.endsWith(".zip"));
for (const [genre, nom] of [["astromosaic", "astromosaic.json"], ["nina", "horizon.hrz"], ["stellarium", zip]]) {
  assert.ok(nom, `export ${genre} absent de ${dossier}`);
  assert.deepEqual(Buffer.from(r.exporter(genre, site)), readFileSync(join(dossier, nom)), genre);
}

// Aperçu, limite et géométrie.
const infos = JSON.parse(r.infos());
assert.equal(infos.largeur, r.largeur);
assert.equal(infos.nord_colonne_px ?? undefined, r.nord);
assert.equal(r.apercu().length, r.apercuLargeur * r.apercuHauteur * 4);
assert.equal(r.limite().length, 2 * r.apercuLargeur);
const colonne = r.tour / 3;
assert.ok(Math.abs(r.colonneDAzimut(r.azimutDeColonne(colonne)) - colonne) < 1e-6, "colonne ↔ azimut");
assert.ok(Math.abs(r.hauteurDeLigne(r.ligneDeHauteur(10)) - 10) < 1e-9, "ligne ↔ hauteur");

// Les erreurs arrivent en exceptions JavaScript.
assert.throws(() => new mm.Releve(new Uint8Array([1, 2, 3]), "rien.jpg"), /photo illisible/);
assert.throws(() => r.exporter("hrz"), /export inconnu/);
assert.throws(() => r.fichierHorizon('{"lat": 44}'), /unknown field `lat`/);

const mo = (sortie.memory.buffer.byteLength / 2 ** 20).toFixed(0);
console.log(
  `ok : ${r.largeur} × ${r.hauteur} px ouverts en ${(duree / 1000).toFixed(1)} s, ` +
    `mémoire du module ${mo} Mo, fichiers identiques à ceux de mmhorizon-calcul`,
);
r.free();
