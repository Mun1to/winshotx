import { getStaticFiles, staticFile } from "remotion";

/**
 * winshotx en Linux, la novedad grande de la 0.6.0: fotos de la app DE VERDAD en un escritorio
 * de Linux, que saca S45 en su laboratorio (Fedora 44 de WSL, aislado del escritorio de Munito,
 * con gnome-shell sin pantalla y el portal de GNOME de verdad). No se pueden sacar con el banco de
 * fotos de `capturas.mjs`, que es Windows y Edge: por eso van aparte, en `public/linux/` (fuera de
 * git, como `public/app/`), y el vídeo coge las que haya.
 *
 * Se piden a 1920 x 1080, la app en inglés y en tema oscuro (BUZON, 20-nov, 13:25). En orden de
 * preferencia: el overlay de captura en GNOME con Wayland (la app funcionando), los ajustes en
 * GNOME, y los ajustes en X11 con xfwm4, que entran después para que se vean los dos escritorios.
 */
type Foto = { nombre: string; ancho: number; alto: number; escritorio: "GNOME" | "X11" };

const PEDIDAS: Foto[] = [
  { nombre: "linux/gnome-recorte.png", ancho: 1920, alto: 1080, escritorio: "GNOME" },
  { nombre: "linux/gnome-ajustes.png", ancho: 1920, alto: 1080, escritorio: "GNOME" },
  { nombre: "linux/x11-ajustes.png", ancho: 1920, alto: 1080, escritorio: "X11" },
];
/**
 * Mientras llegan las pedidas: las del laboratorio de hoy (0.5.2, en español y en tema claro, a
 * 1600 x 1000), solo para montar y medir la escena. No se le enseña a Munito un vídeo con estas.
 */
const PROVISIONALES: Foto[] = [
  { nombre: "linux/provisional-gnome.png", ancho: 1600, alto: 1000, escritorio: "GNOME" },
  { nombre: "linux/provisional-x11.png", ancho: 1600, alto: 1000, escritorio: "X11" },
];

export type FotoDeLinux = Foto & { src: string };

/** Una de GNOME (la mejor que haya) y, si la hay, una de X11. */
export function fotosDeLinux(): { fotos: FotoDeLinux[]; provisionales: boolean } {
  const hay = new Set(getStaticFiles().map((f) => f.name.replace(/\\/g, "/")));
  const elige = (lista: Foto[]) => {
    const gnome = lista.find((f) => f.escritorio === "GNOME" && hay.has(f.nombre));
    const x11 = lista.find((f) => f.escritorio === "X11" && hay.has(f.nombre));
    return [gnome, x11].filter((f): f is Foto => !!f).map((f) => ({ ...f, src: staticFile(f.nombre) }));
  };
  const buenas = elige(PEDIDAS);
  if (buenas.some((f) => f.escritorio === "GNOME")) return { fotos: buenas, provisionales: false };
  return { fotos: elige(PROVISIONALES), provisionales: true };
}
