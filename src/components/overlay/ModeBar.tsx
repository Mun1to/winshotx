import { Camera, Monitor, Video, X } from "lucide-react";
import type { CaptureMode } from "../../lib/types";
import { useT } from "../../lib/i18n";

/**
 * Como reconoce el lienzo un gesto que nace en la barra.
 *
 * La barra ya no se queda los `pointerdown`: se puede empezar a recortar por debajo de
 * ella. El lienzo necesita distinguir ese gesto para no tratar el clic de un boton como
 * un clic suyo, y esta constante es la unica atadura entre los dos lados.
 */
export const SELECTOR_BARRA = "[data-barra-modos]";

interface Props {
  value: CaptureMode;
  onChange: (mode: CaptureMode) => void;
  /** Coger la pantalla entera de un clic, sin arrastrar nada. */
  pantallaEntera: boolean;
  onPantallaEntera: (valor: boolean) => void;
  onCancel: () => void;
  /** Se aparta mientras se arrastra, para no tapar lo que se está recortando. */
  dimmed: boolean;
}

/**
 * Qué se va a hacer con el recorte, elegido ANTES de recortar.
 *
 * Va arriba y centrada, donde Windows pone la suya, porque es el sitio donde la gente
 * ya la busca. Y hace falta: en el perfil "se copia sola" no salía ninguna barra, así
 * que desde ese perfil no había forma de grabar nada.
 *
 * Solo iconos, sin etiqueta: una cámara, una cámara de vídeo y las letras GIF se leen de
 * un vistazo, y tres palabras al lado hacían la barra el doble de ancha para no decir
 * nada más. El texto vive en el tooltip, para quien lo necesite.
 */
export function ModeBar({
  value,
  onChange,
  pantallaEntera,
  onPantallaEntera,
  onCancel,
  dimmed,
}: Props) {
  const t = useT();
  return (
    <div
      data-barra-modos
      className={`group absolute top-6 left-1/2 z-50 -translate-x-1/2 transition-opacity duration-150 ${
        dimmed ? "pointer-events-none opacity-0" : "pointer-events-auto opacity-100 hover:opacity-40"
      }`}
    >
      {/* Al pasar el raton por encima se aparta: pierde el panel, la sombra y el
          desenfoque, y se queda en un fantasma. La franja de arriba del centro es sitio
          de capturar, y un rectangulo negro ahi tapaba justo lo que se iba a recortar.
          Sigue pulsandose igual, y el arrastre la atraviesa (ver SELECTOR_BARRA). */}
      <div className="flex items-center gap-1 rounded-2xl border border-white/10 bg-neutral-900/90 p-1.5 shadow-2xl backdrop-blur-xl transition-[background-color,border-color,box-shadow,backdrop-filter] duration-150 group-hover:border-white/5 group-hover:bg-neutral-900/10 group-hover:shadow-none group-hover:backdrop-blur-none">
        <Boton
          activo={value === "still"}
          onClick={() => onChange("still")}
          etiqueta="Foto"
          titulo="Foto del recorte · F"
        >
          <Camera className="size-[19px]" />
        </Boton>

        <Boton
          activo={value === "video"}
          onClick={() => onChange("video")}
          etiqueta="Vídeo"
          titulo="Grabar el recorte en MP4 · V"
        >
          <Video className="size-[19px]" />
        </Boton>

        <Boton
          activo={value === "gif"}
          onClick={() => onChange("gif")}
          etiqueta="GIF"
          titulo="Grabar el recorte en GIF · G"
        >
          <IconoGif />
        </Boton>

        <span className="mx-1 h-6 w-px bg-white/10" />

        {/* El otro eje: los tres de arriba dicen QUE sale, este dice DE DONDE. Con varias
            pantallas, cada una se pone su numero encima y se coge con un clic. */}
        <Boton
          activo={pantallaEntera}
          onClick={() => onPantallaEntera(!pantallaEntera)}
          etiqueta="Pantalla entera"
          titulo="Pantalla entera, de un clic · P"
        >
          <Monitor className="size-[19px]" />
        </Boton>

        <span className="mx-1 h-6 w-px bg-white/10" />

        <button
          type="button"
          onClick={onCancel}
          title={t("Salir sin capturar · Esc")}
          aria-label={t("Salir sin capturar")}
          className="flex size-9 items-center justify-center rounded-xl text-neutral-400 transition-colors hover:bg-red-500/20 hover:text-red-300"
        >
          <X className="size-[19px]" />
        </button>
      </div>
    </div>
  );
}

/**
 * Las letras GIF dentro de su caja, al peso de los iconos de lucide que tiene al lado.
 *
 * No hay dibujo que signifique "GIF": el que había antes eran unas chispas, que no lo
 * decían. Las letras van como trazos, no como texto, para que engorden con el mismo
 * `stroke-width` que la cámara y no salgan finas a su lado.
 */
function IconoGif() {
  return (
    <svg
      viewBox="0 0 24 24"
      className="size-[19px]"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <rect x="1.6" y="4.4" width="20.8" height="15.2" rx="3.4" />
      {/* Las tres letras, un pelín más finas que la caja: al mismo grosor se empastan a
          los 19 px a los que se ve esto de verdad. */}
      <g strokeWidth="1.7">
        <path d="M9.5 10.2a2.5 2.5 0 0 0-4.4 1.8 2.5 2.5 0 0 0 4.5 1.6v-1.4H8.2" />
        <path d="M12.7 9.7v4.6" />
        <path d="M19 9.7h-3.1v4.6M15.9 12h2.5" />
      </g>
    </svg>
  );
}

function Boton({
  activo,
  onClick,
  etiqueta,
  titulo,
  children,
}: {
  activo: boolean;
  onClick: () => void;
  etiqueta: string;
  titulo: string;
  children: React.ReactNode;
}) {
  const t = useT();
  return (
    <button
      type="button"
      onClick={onClick}
      title={t(titulo)}
      aria-label={t(etiqueta)}
      aria-pressed={activo}
      className={`flex size-9 items-center justify-center rounded-xl transition-colors ${
        activo
          ? "bg-blue-500 text-white shadow-sm"
          : "text-neutral-300 hover:bg-white/10 hover:text-white"
      }`}
    >
      {children}
    </button>
  );
}
