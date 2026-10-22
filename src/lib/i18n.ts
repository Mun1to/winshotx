import { useSyncExternalStore } from "react";

/**
 * Los textos de la aplicacion en doce idiomas.
 *
 * La clave de cada texto es el texto en espannol, no un identificador inventado
 * (`ajustes.capturar.titulo` y demas). Con tantos idiomas y una sola persona escribiendolos,
 * los identificadores solo anaden un diccionario mas que mantener y dejan el codigo
 * ilegible: `t("Ocultar iconos del escritorio")` se entiende sin abrir nada.
 *
 * El precio es que cambiar una palabra en espannol deja esa frase sin traducir hasta que
 * se cambie tambien la clave. A cambio nunca se queda en blanco: lo que no esta en el
 * diccionario sale en espannol, que es peor que traducido pero mucho mejor que vacio.
 */

export type Idioma =
  | "sistema"
  | "es"
  | "en"
  | "zh"
  | "zh-tw"
  | "de"
  | "fr"
  | "it"
  | "ja"
  | "ko"
  | "ru"
  | "pt-br"
  | "pl";

/** Los idiomas que hay, con el nombre que se ensena en el selector. */
export const IDIOMAS: { valor: Idioma; nombre: string }[] = [
  { valor: "sistema", nombre: "Sistema" },
  { valor: "es", nombre: "Espanol" },
  { valor: "en", nombre: "English" },
  { valor: "zh", nombre: "中文（简体）" },
  { valor: "zh-tw", nombre: "中文（繁體）" },
  { valor: "de", nombre: "Deutsch" },
  { valor: "fr", nombre: "Francais" },
  { valor: "it", nombre: "Italiano" },
  { valor: "ja", nombre: "日本語" },
  { valor: "ko", nombre: "한국어" },
  { valor: "ru", nombre: "Русский" },
  { valor: "pt-br", nombre: "Portugues (Brasil)" },
  { valor: "pl", nombre: "Polski" },
];

const CLAVE = "winshotx.idioma";

/**
 * Los doce diccionarios, cada uno detras de un `import()`.
 *
 * **Asi a proposito, y no con un `import` de toda la vida:** los doce juntos son mas de
 * 400 KB, y quien abre la aplicacion en espanol no tiene por que bajarse once idiomas que
 * no va a mirar. Con `import()` Vite parte cada uno en su trozo y solo baja el que toca:
 * el paquete principal pasa de 408 KB a unos 70, que es lo que se veia antes de que
 * existiera el doceavo idioma.
 *
 * El precio es que el diccionario llega un tick despues de que la ventana se monte. Se
 * paga antes del primer pintado (ver `cargar()`), asi que no se ve: si algo falla, lo que
 * hay debajo es el espanol, que es la clave y siempre esta.
 */
const CARGADORES: Record<Exclude<Idioma, "sistema" | "es">, () => Promise<Record<string, string>>> = {
  en: () => import("./textos-en").then((m) => m.EN),
  zh: () => import("./textos-zh").then((m) => m.ZH),
  "zh-tw": () => import("./textos-zh-tw").then((m) => m.ZH_TW),
  de: () => import("./textos-de").then((m) => m.DE),
  fr: () => import("./textos-fr").then((m) => m.FR),
  it: () => import("./textos-it").then((m) => m.IT),
  ja: () => import("./textos-ja").then((m) => m.JA),
  ko: () => import("./textos-ko").then((m) => m.KO),
  ru: () => import("./textos-ru").then((m) => m.RU),
  "pt-br": () => import("./textos-pt-br").then((m) => m.PT_BR),
  pl: () => import("./textos-pl").then((m) => m.PL),
};

/** Lo que hay cargado de cada idioma. El espannol no hace falta: es la clave. */
const cargados = new Map<Exclude<Idioma, "sistema" | "es">, Record<string, string>>();

/** Los que estan viniendo por el `import()`, para no pedir el mismo dos veces. */
const enMarcha = new Map<Exclude<Idioma, "sistema" | "es">, Promise<Record<string, string>>>();

/** El idioma de Windows, que es el que trae el navegador de dentro de la ventana. */
function delSistema(): Exclude<Idioma, "sistema"> {
  const suyo = (navigator.language ?? "en").toLowerCase();
  if (suyo.startsWith("es")) return "es";
  // Chino: el de Taiwan y el de Hong Kong van por tradicional, el resto por simplificado.
  if (suyo.startsWith("zh-tw") || suyo.startsWith("zh-hant") || suyo.startsWith("zh-hk")) return "zh-tw";
  if (suyo.startsWith("zh")) return "zh";
  if (suyo.startsWith("pt")) return "pt-br";
  return suyo.split("-")[0] as Exclude<Idioma, "sistema">;
}

function resolver(idioma: Idioma): Exclude<Idioma, "sistema"> {
  if (idioma !== "sistema") return idioma;
  const detectado = delSistema();
  // Si Windows viene en un idioma que no tenemos, se cae al ingles: es el idioma con el
  // que mas gente se defiende, y el espannol seria un salto raro para quien no lo habla.
  return detectado === "es" || detectado in CARGADORES ? detectado : "en";
}

function leerGuardado(): Idioma {
  try {
    const valor = localStorage.getItem(CLAVE);
    return IDIOMAS.some((i) => i.valor === valor) ? (valor as Idioma) : "sistema";
  } catch {
    return "sistema";
  }
}

let elegido: Idioma = leerGuardado();
let activo: Exclude<Idioma, "sistema"> = resolver(elegido);

/**
 * El diccionario del idioma activo, o nada si todavia no ha llegado.
 *
 * `t` cae al espanol cuando esto es `undefined`, que es lo que pasa durante el tick que
 * el `import()` tarda. Como se carga antes del primer pintado, en la practica no se ve.
 */
function diccionarioActivo(): Record<string, string> | undefined {
  if (activo === "es") return undefined;
  return cargados.get(activo);
}

const oyentes = new Set<() => void>();

function avisar() {
  for (const oyente of oyentes) oyente();
}

/**
 * Trae el diccionario de un idioma, una sola vez.
 *
 * Si ya esta, se devuelve lo que hay. Si esta viniendo, se espera ESE viaje y no se abre
 * otro. Cuando llega, se avisa a quien este mirando para que se repinte.
 */
function cargar(idioma: Exclude<Idioma, "sistema" | "es">): Promise<Record<string, string>> {
  const hecho = cargados.get(idioma);
  if (hecho) return Promise.resolve(hecho);
  const enVuelo = enMarcha.get(idioma);
  if (enVuelo) return enVuelo;

  const viaje = CARGADORES[idioma]()
    .then((diccionario) => {
      cargados.set(idioma, diccionario);
      enMarcha.delete(idioma);
      // Solo se repinta si el idioma que acaba de llegar es el que se esta mirando.
      if (idioma === activo) avisar();
      return diccionario;
    })
    .catch((error) => {
      enMarcha.delete(idioma);
      // Que un idioma falle deja la interfaz en espanol, que es la clave. No se avisa:
      // no hay nada que repintar si no ha llegado nada nuevo.
      console.warn(`[i18n] no se ha podido cargar ${idioma}`, error);
      return {};
    });

  enMarcha.set(idioma, viaje);
  return viaje;
}

/** Cambia el idioma de la ventana y repinta lo que este a la vista. */
export function aplicarIdioma(idioma: Idioma) {
  elegido = idioma;
  const siguiente = resolver(idioma);
  const distinto = siguiente !== activo;
  activo = siguiente;
  document.documentElement.lang = activo;
  try {
    localStorage.setItem(CLAVE, idioma);
  } catch {
    // Que no se pueda recordar no impide usarlo ahora.
  }
  // El diccionario del idioma nuevo se pide aqui y no en el componente: asi el primer
  // pintado despues del cambio ya lo tiene si estaba cargado, y si no, llega en cuanto
  // pueda sin que nadie tenga que esperar a que se abra una pantalla concreta.
  if (activo !== "es") void cargar(activo);
  if (distinto) avisar();
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
  const diccionario = diccionarioActivo();
  const texto = diccionario ? (diccionario[es] ?? es) : es;
  if (!vars) return texto;
  return Object.entries(vars).reduce(
    (frase, [clave, valor]) => frase.split(`{${clave}}`).join(String(valor)),
    texto,
  );
}

export function idiomaActivo(): Exclude<Idioma, "sistema"> {
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

/**
 * Espera a que el diccionario del idioma guardado este dentro.
 *
 * La llaman los puntos de entrada (`main.tsx`, `overlay.tsx`, ...) antes de montar: es lo
 * que hace que el idioma se elija por el que hay en el disco y no por el de Windows, y
 * que la primera frase que se pinta ya venga traducida. Sin esto la ventana saldria un
 * momento en espanol y luego cambiaria sola, que es peor que esperar unos milisegundos.
 */
export function idiomaListo(): Promise<void> {
  const idioma = resolver(leerGuardado());
  if (idioma === "es") return Promise.resolve();
  return cargar(idioma).then(() => undefined);
}

// El idioma activo se carga desde el principio, sin esperar a que alguien pregunte.
if (activo !== "es") void cargar(activo);

document.documentElement.lang = activo;
