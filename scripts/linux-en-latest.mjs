// Las entradas de Linux del latest.json: el AppImage, el .deb y el .rpm que hace la CI
// (`.github/workflows/linux.yml`, el artefacto «winshotx-linux»), cada uno con su `.sig`.
//
// El actualizador de Tauri (2.10 o mas) busca primero la clave de como se instalo la app
// (`linux-x86_64-deb`, `-rpm` o `-appimage`) y, si no la encuentra, `linux-x86_64` a secas. Esa
// ultima apunta al AppImage, que es el unico que se actualiza solo sin pedir contrasenna: el .deb
// y el .rpm se instalan con `pkexec`. Es lo mismo que publica Adeorq desde la 0.9.254.

import { existsSync, readdirSync, readFileSync } from "node:fs";
import { basename, join } from "node:path";
import { comprobarFirma } from "./minisign.mjs";

/** Los tres paquetes, con el nombre exacto que les pone el empaquetador de Tauri. */
export function paquetesDeLinux(version) {
  return [
    { claves: ["linux-x86_64", "linux-x86_64-appimage"], nombre: `winshotx_${version}_amd64.AppImage` },
    { claves: ["linux-x86_64-deb"], nombre: `winshotx_${version}_amd64.deb` },
    { claves: ["linux-x86_64-rpm"], nombre: `winshotx-${version}-1.x86_64.rpm` },
  ];
}

/** Todos los archivos de debajo de `carpeta`: el artefacto trae una subcarpeta por paquete. */
function archivosDe(carpeta) {
  return readdirSync(carpeta, { recursive: true, withFileTypes: true })
    .filter((e) => e.isFile())
    .map((e) => join(e.parentPath ?? e.path, e.name));
}

/**
 * Busca los tres paquetes en `carpeta`, comprueba cada firma con la clave del actualizador y
 * devuelve sus entradas para `platforms`, los archivos que hay que subir a la release y lo que
 * falla. Si `fallos` no esta vacio, no se puede publicar Linux.
 */
export function entradasDeLinux({ carpeta, version, base, clave }) {
  const plataformas = {};
  const archivos = [];
  const fallos = [];
  if (!existsSync(carpeta)) {
    return { plataformas, archivos, fallos: [`No existe la carpeta ${carpeta}`] };
  }
  const todos = archivosDe(carpeta);
  for (const { claves, nombre } of paquetesDeLinux(version)) {
    const encontrados = todos.filter((a) => basename(a) === nombre);
    if (encontrados.length !== 1) {
      // Si esta el de otra version, se dice: es el error de bajar el artefacto de otro commit.
      const extension = nombre.slice(nombre.lastIndexOf("."));
      const otros = todos.map((a) => basename(a)).filter((n) => n.endsWith(extension));
      fallos.push(
        encontrados.length
          ? `Hay ${encontrados.length} copias de ${nombre}: ${encontrados.join(", ")}`
          : `Falta ${nombre}${otros.length ? ` (hay ${otros.join(", ")}: es de otra version)` : ""}`,
      );
      continue;
    }
    const archivo = encontrados[0];
    const sig = `${archivo}.sig`;
    if (!existsSync(sig)) {
      fallos.push(`Falta la firma ${sig}: la CI no tenia la clave puesta`);
      continue;
    }
    const firma = readFileSync(sig, "utf8").trim();
    const resultado = comprobarFirma(readFileSync(archivo), firma, clave);
    if (!resultado.vale) {
      fallos.push(`${nombre}: ${resultado.motivo}`);
      continue;
    }
    for (const plataforma of claves) {
      plataformas[plataforma] = { url: `${base}/${nombre}`, signature: firma };
    }
    archivos.push(archivo, sig);
  }
  return { plataformas, archivos, fallos };
}
