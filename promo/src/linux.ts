import { getStaticFiles, staticFile } from "remotion";

/**
 * winshotx en Linux, la novedad grande de la 0.6.0: fotos de la app DE VERDAD en un escritorio
 * de Linux, que saca S45 en su laboratorio (Fedora 44 de WSL, aislado del escritorio de Munito,
 * con gnome-shell sin pantalla y el portal de GNOME de verdad). No se pueden sacar con el banco de
 * fotos de `capturas.mjs`, que es Windows y Edge: por eso van aparte, en `public/linux/` (fuera de
 * git, como `public/app/`).
 *
 * Las de la 0.6.0 (S45, 20-nov, 16:41, binario de `release/0.6.0` en `e7c6152`): 1920 x 1080, la
 * app en inglés y en oscuro y los atajos puestos. En orden de preferencia, la pantalla de captura
 * en GNOME con Wayland (la app funcionando: una zona elegida y la barra debajo), los ajustes en
 * GNOME, y los ajustes en X11 (con openbox y el fondo negro), que entra después para que se vean
 * los dos escritorios.
 */
type Caja = { x: number; y: number; w: number; h: number };
type Foto = { nombre: string; ancho: number; alto: number; escritorio: "GNOME" | "X11"; tapar?: Caja[] };

/**
 * La pastilla naranja de arriba a la derecha de la barra de GNOME es su aviso de «se está
 * grabando la pantalla», y sale por cómo hizo S45 la foto (no la pone winshotx): se tapa con el
 * negro de la barra. Medida en las dos fotos de GNOME: de x 1813 a 1868 y de y 3 a 28, con la
 * barra en negro puro de y 0 a 31; el icono de al lado empieza en x 1885 y no se toca.
 */
const PASTILLA_DE_GNOME: Caja = { x: 1808, y: 0, w: 66, h: 32 };

const FOTOS: Foto[] = [
  { nombre: "linux/gnome-recorte.png", ancho: 1920, alto: 1080, escritorio: "GNOME", tapar: [PASTILLA_DE_GNOME] },
  { nombre: "linux/gnome-ajustes.png", ancho: 1920, alto: 1080, escritorio: "GNOME", tapar: [PASTILLA_DE_GNOME] },
  { nombre: "linux/x11-ajustes.png", ancho: 1920, alto: 1080, escritorio: "X11" },
];

export type FotoDeLinux = Foto & { src: string };

/**
 * Una de GNOME (la mejor que haya) y, si la hay, una de X11. Sin ninguna de GNOME el render se
 * para: el vídeo no sale con la escena de Linux vacía.
 */
export function fotosDeLinux(): FotoDeLinux[] {
  const hay = new Set(getStaticFiles().map((f) => f.name.replace(/\\/g, "/")));
  const gnome = FOTOS.find((f) => f.escritorio === "GNOME" && hay.has(f.nombre));
  const x11 = FOTOS.find((f) => f.escritorio === "X11" && hay.has(f.nombre));
  if (!gnome) throw new Error("faltan las fotos de Linux en public/linux/ (gnome-recorte.png o gnome-ajustes.png): ver src/linux.ts");
  return [gnome, x11].filter((f): f is Foto => !!f).map((f) => ({ ...f, src: staticFile(f.nombre) }));
}
