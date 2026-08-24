/**
 * Avisa a los buscadores de que la web cambio, sin esperar a que pasen ellos.
 *
 *   node frontlaxweb/avisar-buscadores.mjs
 *
 * Usa IndexNow, el protocolo que comparten Bing, Yandex, Naver y Seznam: se manda
 * la lista de URLs y la clave que esta publicada en la propia web, y ellos comprueban
 * que la clave existe antes de hacer caso. Google NO usa IndexNow: ese va por Search
 * Console, que hay que abrir a mano.
 *
 * La clave vive en frontlaxweb/.indexnow-clave y su archivo publico es
 * frontlaxweb/<clave>.txt. Si se borra el archivo publico, IndexNow deja de aceptar
 * los avisos.
 */
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const aqui = dirname(fileURLToPath(import.meta.url));
const dominio = readFileSync(join(aqui, "CNAME"), "utf8").trim();
const clave = readFileSync(join(aqui, ".indexnow-clave"), "utf8").trim();

// Las URLs salen del sitemap, que ya es la lista buena. Escribirlas aqui otra vez
// significa que el dia que se anada una pagina, este script avisa de menos y encima
// dice que ha ido bien.
const sitemap = readFileSync(join(aqui, "sitemap.xml"), "utf8");
const urls = [...sitemap.matchAll(/<loc>([^<]+)<\/loc>/g)].map(([, u]) => u.trim());
if (!urls.length) {
  console.error("sitemap.xml no tiene ninguna URL.");
  process.exit(1);
}
const ajenas = urls.filter((u) => !u.startsWith(`https://${dominio}/`));
if (ajenas.length) {
  console.error(`El sitemap tiene URLs de otro dominio: ${ajenas.join(", ")}`);
  process.exit(1);
}

const cuerpo = {
  host: dominio,
  key: clave,
  keyLocation: `https://${dominio}/${clave}.txt`,
  urlList: urls,
};

// Primero comprobar que la clave esta publicada, porque si no el aviso se rechaza
// sin decir por que.
const comprobacion = await fetch(cuerpo.keyLocation);
if (!comprobacion.ok) {
  console.error(`La clave no esta publicada en ${cuerpo.keyLocation} (${comprobacion.status}).`);
  console.error("Sube la web primero y vuelve a intentarlo.");
  process.exit(1);
}
const publicada = (await comprobacion.text()).trim();
if (publicada !== clave) {
  console.error("La clave publicada no coincide con la local.");
  process.exit(1);
}

const respuesta = await fetch("https://api.indexnow.org/indexnow", {
  method: "POST",
  headers: { "Content-Type": "application/json; charset=utf-8" },
  body: JSON.stringify(cuerpo),
});

// 200 y 202 son los dos "recibido". El 202 significa que la clave esta en revision.
if (respuesta.ok) {
  console.log(`Avisadas ${urls.length} URLs (respuesta ${respuesta.status}).`);
  for (const u of urls) console.log("  " + u);
} else {
  console.error(`IndexNow respondio ${respuesta.status}: ${await respuesta.text()}`);
  process.exit(1);
}
