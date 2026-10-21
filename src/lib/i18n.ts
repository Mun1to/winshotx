import { useSyncExternalStore } from "react";
import { EN } from "./textos-en";
import { ZH } from "./textos-zh";
import { DE } from "./textos-de";
import { FR } from "./textos-fr";
import { IT } from "./textos-it";
import { PTBR } from "./textos-pt-br";
import { RU } from "./textos-ru";
import { JA } from "./textos-ja";
import { KO } from "./textos-ko";
import { ZHTW } from "./textos-zh-tw";
import { NL } from "./textos-nl";
import { PL } from "./textos-pl";
import { TR } from "./textos-tr";
import { UK } from "./textos-uk";
import { SV } from "./textos-sv";
import { FI } from "./textos-fi";
import { DA } from "./textos-da";

/**
 * Los textos de la aplicacion en dieciocho idiomas.
 *
 * La clave de cada texto es el texto en espannol, no un identificador inventado
 * (`ajustes.capturar.titulo` y demas). Con dos idiomas y una sola persona escribiendolos,
 * los identificadores solo anaden un diccionario mas que mantener y dejan el codigo
 * ilegible: `t("Ocultar iconos del escritorio")` se entiende sin abrir nada.
 *
 * El precio es que cambiar una palabra en espannol deja esa frase sin traducir hasta que
 * se cambie tambien la clave. A cambio nunca se queda en blanco: lo que no esta en el
 * diccionario sale en espannol, que es peor que traducido pero mucho mejor que vacio.
 *
 * Con dieciocho idiomas, el `activo === "zh" ? ZH : EN` de antes ya no cabe: cada idioma
 * nuevo obliga a tocar el traductor. Por eso los catalogos viven en un registro
 * (`CATALOGOS`) y el traductor solo busca en el. Anadir un idioma es un archivo mas y una
 * linea mas en el registro, sin tocar nada mas.
 */

/** Un idioma que la aplicacion sabe hablar. */
export type Idioma = "sistema" | CodigoIdioma;

/** Los codigos de los idiomas traducidos, sin el «sistema». */
export type CodigoIdioma =
  | "es"
  | "en"
  | "zh"
  | "de"
  | "fr"
  | "it"
  | "pt-br"
  | "ru"
  | "ja"
  | "ko"
  | "zh-tw"
  | "nl"
  | "pl"
  | "tr"
  | "uk"
  | "sv"
  | "fi"
  | "da";

/**
 * Los catalogos, por idioma.
 *
 * El espanol no esta aqui: es la clave, asi que no necesita traduccion (ver `t`).
 */
const CATALOGOS: Record<CodigoIdioma, Record<string, string>> = {
  en: EN,
  zh: ZH,
  de: DE,
  fr: FR,
  it: IT,
  "pt-br": PTBR,
  ru: RU,
  ja: JA,
  ko: KO,
  "zh-tw": ZHTW,
  nl: NL,
  pl: PL,
  tr: TR,
  uk: UK,
  sv: SV,
  fi: FI,
  da: DA,
};

/**
 * Que codigo de idioma corresponde a cada etiqueta de Windows.
 *
 * Windows escribe el idioma como `es-ES`, `zh-CN`, `pt-BR`... La parte de delante es el
 * idioma y la de detras el pais. Aqui se decide que pais cuenta: el chino simplificado
 * `zh-CN` y el chino tradicional `zh-TW` son dos idiomas distintos, y el portugues de
 * Portugal `pt-PT` se habla distinto del de Brasil `pt-BR`, asi que el pais decide.
 *
 * El orden importa: se busca la coincidencia mas larga primero, para que `zh-TW` no se lo
 * coma el `zh` que esta antes.
 */
const POR_REGION: { codigo: CodigoIdioma; region: string }[] = [
  { codigo: "zh-tw", region: "zh-hant" },
  { codigo: "zh-tw", region: "zh-tw" },
  { codigo: "zh-tw", region: "zh-hk" },
  { codigo: "zh-tw", region: "zh-mo" },
  { codigo: "zh", region: "zh-hans" },
  { codigo: "pt-br", region: "pt-br" },
];

/** Los codigos que Windows escribe para los idiomas de la lista. */
const CODIGOS_SISTEMA: { codigo: CodigoIdioma; etiqueta: string }[] = [
  { codigo: "es", etiqueta: "es" },
  { codigo: "en", etiqueta: "en" },
  { codigo: "zh", etiqueta: "zh-cn" },
  { codigo: "zh-tw", etiqueta: "zh-tw" },
  { codigo: "de", etiqueta: "de" },
  { codigo: "fr", etiqueta: "fr" },
  { codigo: "it", etiqueta: "it" },
  { codigo: "pt-br", etiqueta: "pt-br" },
  { codigo: "ru", etiqueta: "ru" },
  { codigo: "ja", etiqueta: "ja" },
  { codigo: "ko", etiqueta: "ko" },
  { codigo: "nl", etiqueta: "nl" },
  { codigo: "pl", etiqueta: "pl" },
  { codigo: "tr", etiqueta: "tr" },
  { codigo: "uk", etiqueta: "uk" },
  { codigo: "sv", etiqueta: "sv" },
  { codigo: "fi", etiqueta: "fi" },
  { codigo: "da", etiqueta: "da" },
];

const CLAVE = "winshotx.idioma";

/**
 * El idioma de Windows, que es el que trae el navegador de dentro de la ventana.
 *
 * Se mira primero la etiqueta completa (`zh-TW`) y despues la parte de delante (`zh`),
 * que es la que dice el idioma sin decir el pais.
 */
function delSistema(): CodigoIdioma {
  const suyo = (navigator.language ?? "en").toLowerCase();
  for (const { codigo, region } of POR_REGION) {
    if (suyo === region || suyo.startsWith(`${region}-`)) return codigo;
  }
  for (const { codigo, etiqueta } of CODIGOS_SISTEMA) {
    if (suyo === etiqueta || suyo.startsWith(`${etiqueta}-`)) return codigo;
  }
  return "en";
}

function resolver(idioma: Idioma): CodigoIdioma {
  return idioma === "sistema" ? delSistema() : idioma;
}

function leerGuardado(): Idioma {
  try {
    const valor = localStorage.getItem(CLAVE);
    return esIdiomaConocido(valor) ? valor : "sistema";
  } catch {
    return "sistema";
  }
}

/** Si un valor cualquiera es un idioma que la aplicacion conoce. */
export function esIdiomaConocido(valor: string | null | undefined): valor is Idioma {
  if (!valor) return false;
  if (valor === "sistema") return true;
  return Object.prototype.hasOwnProperty.call(CATALOGOS, valor);
}

// Igual que el tema: se recuerda en el navegador para que la primera pintada ya salga en
// el idioma bueno, sin esperar al viaje hasta Rust y sin que las frases cambien delante.
let elegido: Idioma = leerGuardado();
let activo: CodigoIdioma = resolver(elegido);

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
  const diccionario = CATALOGOS[activo] ?? EN;
  const texto = activo === "es" ? es : (diccionario[es] ?? es);
  if (!vars) return texto;
  return Object.entries(vars).reduce(
    (frase, [clave, valor]) => frase.split(`{${clave}}`).join(String(valor)),
    texto,
  );
}

export function idiomaActivo(): CodigoIdioma {
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
