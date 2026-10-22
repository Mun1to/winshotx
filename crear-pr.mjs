#!/usr/bin/env node
/**
 * Crea el pull request en el repositorio de upstream con la API de GitHub.
 *
 * El PR sale de la rama feature/more-languages de wang13zhe/winshotx hacia
 * Mun1to/winshotx. La descripcion va en ingles, como el resto del repositorio.
 */
import { readFileSync } from "node:fs";

const TOKEN = readFileSync("C:/Users/Admin/.dsh/gh-token.txt", "utf8").trim();

const CABECERA = {
  title: "Speak twelve languages, and download only the one in use",
  head: "wang13zhe:feature/more-languages",
  base: "main",
  body: `The app speaks Spanish, English and Chinese. This makes it twelve: German, French, Italian, Japanese, Korean, Russian, Brazilian Portuguese, Traditional Chinese and Polish join them.

**What is in here**

- Nine dictionaries, one per new language, each with every phrase the English one has.
- The setting that picks a language now lists all twelve, each by the name it gives itself.
- The dictionaries are no longer bundled together. Twelve of them are more than 400 KB, and someone reading Spanish had no business downloading eleven languages they will never look at. Each one now goes behind its own \`import()\`, so the main bundle drops back to about 70 KB and the language in use arrives on its own. The entry points wait for it before painting, so nothing on screen changes language after the fact.
- The health checks a dictionary gets now run over all twelve, in a loop, so adding a language later is one line rather than another copy of the same block.
- \`node scripts/ver-idiomas.mjs\` walks every dictionary and says what is mechanically wrong with it: a missing phrase, a lost \`{placeholder}\`, a key nobody uses, a brand name that got capitalised.
- \`docs/TRADUCCION-NORMAS.md\`: what a translation may not contain, and how long a piece of interface text should be.

**About the translations themselves**

They were written to be read rather than translated. The brand name stays lowercase and untranslated. Key names go by what the keyboard actually prints, which is why the Shift key says \`Shift\` in all twelve and not \`Mayús\` — that is what a Spanish keyboard prints on it, and a Japanese or Russian keyboard does not. Every \`{placeholder}\` survives with its original count.

Nine translators worked from the rules in the document, and each dictionary was read through again afterwards by someone who writes that language, which is where the wording of "ring buffer" stopped being a piece of jewellery in three of them.

**One unrelated fix, in its own commit**

\`src/test/preparar.ts\` restores happy-dom's \`localStorage\` over the one Node 26 installs globally. Without it every frontend test that touches storage fails on Node 26, before and after this branch, and that was true of the repository as it stood.

The signing key is untouched and the updater still points where it pointed.`,
};

const respuesta = await fetch("https://api.github.com/repos/Mun1to/winshotx/pulls", {
  method: "POST",
  headers: {
    Authorization: `Bearer ${TOKEN}`,
    Accept: "application/vnd.github+json",
    "Content-Type": "application/json",
    "User-Agent": "winshotx-fork",
  },
  body: JSON.stringify(CABECERA),
});

const cuerpo = await respuesta.text();
let datos;
try {
  datos = JSON.parse(cuerpo);
} catch {
  console.log("respuesta no JSON:", cuerpo.slice(0, 500));
  process.exit(1);
}

if (!respuesta.ok) {
  console.log("HTTP", respuesta.status);
  console.log(datos.message ?? cuerpo.slice(0, 500));
  if (datos.errors) console.log(JSON.stringify(datos.errors, null, 2));
  process.exit(1);
}

console.log("PR creado:", datos.html_url);
console.log("numero:", datos.number);
console.log("estado:", datos.state);
