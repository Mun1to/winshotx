import type { Theme } from "./types";

/**
 * De que color se pinta la ventana. El CSS trae el tema oscuro puesto y solo cambia
 * cuando el `<html>` lleva `data-tema="claro"`, asi que aqui todo el trabajo es decidir
 * cual de los dos toca y escribir ese atributo.
 */

const CLAVE = "winshotx.tema";
const CONSULTA = "(prefers-color-scheme: dark)";

const sistema = window.matchMedia(CONSULTA);

/**
 * Se lee del navegador antes que de los ajustes a proposito: los ajustes tardan un viaje
 * de ida y vuelta hasta Rust, y en ese rato la ventana ya se ha pintado. Sin esto, quien
 * tenga el tema claro forzado ve un fogonazo oscuro en cada arranque.
 */
let elegido = leerGuardado();

function leerGuardado(): Theme {
  try {
    const valor = localStorage.getItem(CLAVE);
    return valor === "claro" || valor === "oscuro" ? valor : "sistema";
  } catch {
    // Sin almacenamiento se sigue igual, solo que con el fogonazo.
    return "sistema";
  }
}

function pintar() {
  const oscuro = elegido === "oscuro" || (elegido === "sistema" && sistema.matches);
  document.documentElement.dataset.tema = oscuro ? "oscuro" : "claro";
}

/** Cambia el tema de la ventana y se acuerda de el para el proximo arranque. */
export function aplicarTema(tema: Theme) {
  elegido = tema;
  try {
    localStorage.setItem(CLAVE, tema);
  } catch {
    // Que no se pueda recordar no impide pintarlo ahora.
  }
  pintar();
}

// Con el tema en automatico hay que seguir a Windows mientras la ventana esta abierta,
// no solo al arrancar: el cambio de claro a oscuro al atardecer lo hace el sistema solo.
sistema.addEventListener("change", pintar);

pintar();
