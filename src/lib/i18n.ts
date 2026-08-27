import { useSyncExternalStore } from "react";
import { EN } from "./textos-en";

/**
 * Los textos de la aplicacion en dos idiomas.
 *
 * La clave de cada texto es el texto en espannol, no un identificador inventado
 * (`ajustes.capturar.titulo` y demas). Con dos idiomas y una sola persona escribiendolos,
 * los identificadores solo anaden un diccionario mas que mantener y dejan el codigo
 * ilegible: `t("Ocultar iconos del escritorio")` se entiende sin abrir nada.
 *
 * El precio es que cambiar una palabra en espannol deja esa frase sin traducir hasta que
 * se cambie tambien la clave. A cambio nunca se queda en blanco: lo que no esta en el
 * diccionario sale en espannol, que es peor que traducido pero mucho mejor que vacio.
 */

export type Idioma = "sistema" | "es" | "en";

const CLAVE = "winshotx.idioma";

/** El idioma de Windows, que es el que trae el navegador de dentro de la ventana. */
function delSistema(): "es" | "en" {
  const suyo = navigator.language ?? "en";
  return suyo.toLowerCase().startsWith("es") ? "es" : "en";
}

function resolver(idioma: Idioma): "es" | "en" {
  return idioma === "sistema" ? delSistema() : idioma;
}

function leerGuardado(): Idioma {
  try {
    const valor = localStorage.getItem(CLAVE);
    return valor === "es" || valor === "en" ? valor : "sistema";
  } catch {
    return "sistema";
  }
}

// Igual que el tema: se recuerda en el navegador para que la primera pintada ya salga en
// el idioma bueno, sin esperar al viaje hasta Rust y sin que las frases cambien delante.
let elegido: Idioma = leerGuardado();
let activo: "es" | "en" = resolver(elegido);

const oyentes = new Set<() => void>();

function avisar() {
  for (const oyente of oyentes) oyente();
}

/** Cambia el idioma de la ventana y repinta lo que este a la vista. */
export function aplicarIdioma(idioma: Idioma) {
  elegido = idioma;
  activo = resolver(idioma);
  document.documentElement.lang = activo;
  try {
    localStorage.setItem(CLAVE, idioma);
  } catch {
    // Que no se pueda recordar no impide usarlo ahora.
  }
  avisar();
}

/**
 * El texto en el idioma de ahora. Se le pasa la frase en espannol.
 *
 * Lo que cambia (un numero, una version, una carpeta) va con marcadores `{asi}` y nunca
 * partiendo la frase en trozos: el orden de las palabras cambia de un idioma a otro, y una
 * frase cosida a cachos solo se puede traducir bien por casualidad.
 */
export function t(es: string, vars?: Record<string, string | number>): string {
  const texto = activo === "es" ? es : (EN[es] ?? es);
  if (!vars) return texto;
  return Object.entries(vars).reduce(
    (frase, [clave, valor]) => frase.split(`{${clave}}`).join(String(valor)),
    texto,
  );
}

export function idiomaActivo(): "es" | "en" {
  return activo;
}

function suscribir(oyente: () => void) {
  oyentes.add(oyente);
  return () => {
    oyentes.delete(oyente);
  };
}

/**
 * Lo que usan los componentes. Devuelve `t`, y ademas deja al componente apuntado para
 * que se repinte solo cuando se cambia de idioma: sin esto habria que recargar la ventana
 * entera, y se perderia por donde iba el usuario.
 */
export function useT() {
  useSyncExternalStore(suscribir, idiomaActivo, idiomaActivo);
  return t;
}

document.documentElement.lang = activo;
