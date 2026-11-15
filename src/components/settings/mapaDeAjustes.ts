import {
  Camera,
  History,
  ImageDown,
  Images,
  Keyboard,
  LifeBuoy,
  SlidersHorizontal,
  Video,
  type LucideIcon,
} from "lucide-react";

/**
 * Los ocho apartados de los ajustes, en el orden del riel de la izquierda (S36, la
 * variación 9 que eligió Munito el 15-nov-2026).
 *
 * Antes eran cinco pestañas arriba con dos columnas de tarjetas cada una; ahora es un
 * apartado a la vez, con su título, su frase y sus filas en medio, y a la derecha el dibujo
 * de cómo va a salir. Por eso Capturar se parte en dos (lo que pasa al pulsar y lo que pasa
 * con la foto), Grabar en dos (grabar y los últimos segundos), y La app en dos (la app y la
 * ayuda): cada uno cabe entero sin rueda y su dibujo cuenta una sola cosa.
 */
export type ApartadoId =
  | "capturar"
  | "despues"
  | "grabar"
  | "ultimos"
  | "historial"
  | "teclas"
  | "app"
  | "ayuda";

export interface Apartado {
  id: ApartadoId;
  /** El nombre, que es también lo que dice el icono al dejar el ratón encima. */
  nombre: string;
  icono: LucideIcon;
  /** Una frase debajo del título. El historial no lleva: es una galería, no una lista de ajustes. */
  frase: string;
  /** Si lleva el dibujo de «Así va a salir» a la derecha. El historial y la ayuda, no. */
  dibujo: boolean;
}

export const APARTADOS: Apartado[] = [
  {
    id: "capturar",
    nombre: "Capturar",
    icono: Camera,
    frase: "Los atajos y lo que pasa al pulsarlos.",
    dibujo: true,
  },
  {
    id: "despues",
    nombre: "Después de capturar",
    icono: ImageDown,
    frase: "Qué se hace con la foto en cuanto la tienes.",
    dibujo: true,
  },
  {
    id: "grabar",
    nombre: "Grabar",
    icono: Video,
    frase: "El atajo, la fluidez, el sonido y la cámara.",
    dibujo: true,
  },
  {
    id: "ultimos",
    nombre: "Los últimos segundos",
    icono: History,
    frase: "Graba siempre en memoria y te quedas con lo último cuando pasa algo.",
    dibujo: true,
  },
  { id: "historial", nombre: "Historial", icono: Images, frase: "", dibujo: false },
  {
    id: "teclas",
    nombre: "Teclas de Windows",
    icono: Keyboard,
    frase: "Qué teclas del sistema abren winshotx en vez de Windows.",
    dibujo: true,
  },
  {
    id: "app",
    nombre: "La app",
    icono: SlidersHorizontal,
    frase: "Dónde guarda, cómo se ve y cuándo arranca.",
    dibujo: true,
  },
  {
    id: "ayuda",
    nombre: "Ayuda y winshotx",
    icono: LifeBuoy,
    frase: "La bienvenida, el tour y de dónde sale winshotx.",
    dibujo: false,
  },
];

export const apartado = (id: ApartadoId): Apartado => APARTADOS.find((a) => a.id === id) ?? APARTADOS[0];
