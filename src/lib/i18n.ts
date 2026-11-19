import { useSyncExternalStore } from "react";
import { EN } from "./textos-en";
import { ZH } from "./textos-zh";

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

export type Idioma = "sistema" | "es" | "en" | "zh";

const CLAVE = "winshotx.idioma";

/** El idioma de Windows, que es el que trae el navegador de dentro de la ventana. */
function delSistema(): "es" | "en" | "zh" {
  const suyo = (navigator.language ?? "en").toLowerCase();
  if (suyo.startsWith("es")) return "es";
  if (suyo.startsWith("zh")) return "zh";
  return "en";
}

function resolver(idioma: Idioma): "es" | "en" | "zh" {
  return idioma === "sistema" ? delSistema() : idioma;
}

function leerGuardado(): Idioma {
  try {
    const valor = localStorage.getItem(CLAVE);
    return valor === "es" || valor === "en" || valor === "zh" ? valor : "sistema";
  } catch {
    return "sistema";
  }
}

// Igual que el tema: se recuerda en el navegador para que la primera pintada ya salga en
// el idioma bueno, sin esperar al viaje hasta Rust y sin que las frases cambien delante.
let elegido: Idioma = leerGuardado();
let activo: "es" | "en" | "zh" = resolver(elegido);

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
  // El espannol es la clave del diccionario, asi que no necesita traduccion, y lo que
  // falte en otro idioma sale en espannol: peor que traducido, mucho mejor que vacio.
  const diccionario = activo === "zh" ? ZH : EN;
  const texto = activo === "es" ? es : (diccionario[es] ?? es);
  if (!vars) return texto;
  return Object.entries(vars).reduce(
    (frase, [clave, valor]) => frase.split(`{${clave}}`).join(String(valor)),
    texto,
  );
}

/** Lo que separa la frase de un error de la orden de terminal con que acaba. */
const ANTES_DE_LA_ORDEN = " Se instala con: ";

/**
 * Un error que llega de Rust. Su texto en espannol es la clave, como el de cualquier frase,
 * salvo cuando acaba en una orden de terminal («Para leer el texto falta Tesseract. Se instala
 * con: sudo apt install tesseract-ocr»): la orden cambia con la distribucion y con el idioma
 * del equipo, asi que se traduce la frase de delante y la orden se deja tal cual.
 */
export function tError(error: unknown): string {
  const texto = String(error);
  const corte = texto.indexOf(ANTES_DE_LA_ORDEN);
  if (corte < 0) return t(texto);
  const orden = texto.slice(corte + ANTES_DE_LA_ORDEN.length);
  return `${t(texto.slice(0, corte))} ${t("Se instala con:")} ${orden}`;
}

/**
 * Un aviso ya pasado por `tError`, partido en su frase y la orden de terminal con que acaba; o
 * nada si no lleva orden. Los que la llevan no se cortan con puntos suspensivos y se quedan
 * `DURA_CON_ORDEN`: la orden hay que llegar a copiarla, y de «Se instala con: su…» no se saca
 * nada.
 */
export function partirOrden(aviso: string): { frase: string; orden: string } | null {
  const marca = ` ${t("Se instala con:")} `;
  const corte = aviso.indexOf(marca);
  if (corte < 0) return null;
  return { frase: aviso.slice(0, corte + marca.length - 1), orden: aviso.slice(corte + marca.length) };
}

/** Lo que se queda a la vista un aviso con una orden de terminal: lo que se tarda en teclearla. */
export const DURA_CON_ORDEN = 20_000;

export function idiomaActivo(): "es" | "en" | "zh" {
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
