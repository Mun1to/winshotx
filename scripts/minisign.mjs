// Lee y comprueba las firmas del actualizador, que son de minisign: lo que usa Tauri.
//
//   - La clave publica de `tauri.conf.json` es base64 de un archivo de clave publica entero
//     (con su comentario). Dentro: 2 bytes de algoritmo, 8 de identificador y 32 de clave.
//   - El `.sig` es base64 de un archivo de firma entero. Dentro: 2 bytes de algoritmo, 8 de
//     identificador y 64 de firma, y despues un comentario de confianza con su propia firma
//     (la «global»), que firma los 64 bytes de antes seguidos del comentario.
//   - `Ed` firma el contenido tal cual; `ED` firma su BLAKE2b-512. Tauri usa la segunda.
//
// El actualizador comprueba las dos firmas. Si cualquiera falla, rechaza la descarga sin
// avisar a nadie: el boton de actualizar simplemente no acaba nunca.

import { createHash, createPublicKey, verify } from "node:crypto";

/** Las lineas de un archivo minisign que viene en base64, sin las vacias. */
function lineas(base64Entero) {
  return Buffer.from(base64Entero, "base64")
    .toString("utf8")
    .split("\n")
    .map((l) => l.trim())
    .filter(Boolean);
}

/** Algoritmo, identificador y datos de la primera linea en base64 de una clave o una firma. */
function cuerpo(base64Entero, bytesFinales) {
  const linea = lineas(base64Entero).find((l) => !l.includes("comment:"));
  const cru = Buffer.from(linea ?? "", "base64");
  return {
    algoritmo: cru.subarray(0, 2).toString("latin1"),
    id: cru.subarray(2, 10).toString("hex"),
    dato: cru.subarray(10, 10 + bytesFinales),
  };
}

/** La clave publica de `plugins.updater.pubkey`. */
export function leerClave(base64Entero) {
  return cuerpo(base64Entero, 32);
}

/** Un `.sig` de Tauri, con su comentario de confianza y la firma global. */
export function leerFirma(base64Entero) {
  const todas = lineas(base64Entero);
  const i = todas.findIndex((l) => l.startsWith("trusted comment: "));
  return {
    ...cuerpo(base64Entero, 64),
    comentario: i >= 0 ? todas[i].slice("trusted comment: ".length) : null,
    global: i >= 0 && todas[i + 1] ? Buffer.from(todas[i + 1], "base64") : null,
  };
}

/** La clave de ed25519 en el formato que entiende Node. */
function claveDeNode(bytes32) {
  const der = Buffer.concat([Buffer.from("302a300506032b6570032100", "hex"), bytes32]);
  return createPublicKey({ key: der, format: "der", type: "spki" });
}

/**
 * Comprueba, como lo hara el actualizador, que `firmaB64` (el contenido de un `.sig`) firma
 * `bytes` con la clave `claveB64`. Devuelve `{ vale: true }` o `{ vale: false, motivo }`.
 */
export function comprobarFirma(bytes, firmaB64, claveB64) {
  const clave = leerClave(claveB64);
  const firma = leerFirma(firmaB64);
  if (firma.dato.length !== 64 || clave.dato.length !== 32) {
    return { vale: false, motivo: "la firma o la clave no tienen el formato de minisign" };
  }
  // Si los identificadores no coinciden, la firma es de OTRA clave: lo que pasa cuando se firma
  // con una clave distinta de la que lleva la app compilada.
  if (firma.id !== clave.id) {
    return { vale: false, motivo: `la firma es de otra clave (${firma.id}, la app lleva ${clave.id})` };
  }
  const nodo = claveDeNode(clave.dato);
  const mensaje = firma.algoritmo === "ED" ? createHash("blake2b512").update(bytes).digest() : bytes;
  if (!verify(null, mensaje, nodo, firma.dato)) {
    return { vale: false, motivo: "la firma no es de este archivo (es de otra compilacion)" };
  }
  if (firma.comentario === null || !firma.global) {
    return { vale: false, motivo: "a la firma le falta el comentario de confianza" };
  }
  const global = Buffer.concat([firma.dato, Buffer.from(firma.comentario, "utf8")]);
  if (!verify(null, global, nodo, firma.global)) {
    return { vale: false, motivo: "el comentario de confianza de la firma esta alterado" };
  }
  return { vale: true };
}
