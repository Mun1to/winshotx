import type { Language } from "./types";

/**
 * Los idiomas que habla la aplicacion.
 *
 * El nombre de cada idioma va en ESE idioma, que es como lo busca quien lo necesita:
 * alguien que abre la aplicacion en chino no esta buscando la palabra «Chino».
 *
 * Vive en su propio archivo porque lo usan dos sitios —la seccion de ajustes y la barra
 * de abajo— y duplicar la lista hacia que se desincronizasen, que es como se rompio la
 * de arriba cuando se cambio el tema.
 */
export const IDIOMAS: { value: Language; label: string }[] = [
  { value: "sistema", label: "Automático" },
  { value: "es", label: "Español" },
  { value: "en", label: "English" },
  { value: "zh", label: "简体中文" },
  { value: "zh-tw", label: "繁體中文" },
  { value: "de", label: "Deutsch" },
  { value: "fr", label: "Français" },
  { value: "it", label: "Italiano" },
  { value: "pt-br", label: "Português (Brasil)" },
  { value: "ru", label: "Русский" },
  { value: "ja", label: "日本語" },
  { value: "ko", label: "한국어" },
  { value: "nl", label: "Nederlands" },
  { value: "pl", label: "Polski" },
  { value: "tr", label: "Türkçe" },
  { value: "uk", label: "Українська" },
  { value: "sv", label: "Svenska" },
  { value: "fi", label: "Suomi" },
  { value: "da", label: "Dansk" },
];

/**
 * Las siglas cortas, para los sitios donde no cabe el nombre entero.
 *
 * La barra de abajo es uno: con dieciocho idiomas, un botón por idioma no cabe ni de ancho
 * ni de ancho de letra. Se enseña el de dos letras y el nombre va en el globo y en el
 * nombre accesible.
 */
export const SIGLAS: Record<Language, string> = {
  sistema: "AUTO",
  es: "ES",
  en: "EN",
  zh: "简",
  "zh-tw": "繁",
  de: "DE",
  fr: "FR",
  it: "IT",
  "pt-br": "PT",
  ru: "RU",
  ja: "JA",
  ko: "KO",
  nl: "NL",
  pl: "PL",
  tr: "TR",
  uk: "UK",
  sv: "SV",
  fi: "FI",
  da: "DA",
};
