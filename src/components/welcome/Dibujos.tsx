import type { ReactNode } from "react";
import {
  ArrowRight,
  ChevronUp,
  Copy,
  Download,
  Pencil,
  Pin,
  Scissors,
  Volume2,
  Wifi,
  X,
} from "lucide-react";
import { useT } from "../../lib/i18n";
import { Marca } from "../ui/Marca";

/**
 * Los dibujos de «Así va a salir» de la bienvenida (S38, 15-nov-2026).
 *
 * Es el idioma que eligió Munito para los ajustes de la 0.5 (la variación 9): al lado de lo
 * que se elige, un dibujo pequeño de lo que va a pasar, que cambia en cuanto se cambia la
 * elección. Quien acaba de instalar no sabe qué es «Con barra» ni «Al vuelo»; viéndolo, sí.
 *
 * Son dibujos y no fotos, y no llevan letras: una letra de 8 px dentro de una pantallita no
 * se lee, y la frase que lo cuenta va debajo, a tamaño de leer. Pintan Windows, así que no
 * cambian con el tema de la app: el escritorio es el mismo en claro y en oscuro.
 */

const ESCRITORIO = "linear-gradient(140deg, #1d4ed8 0%, #0f766e 55%, #155e75 100%)";

/** Una ventana clara cualquiera, con su barra de título y unas líneas de texto. */
function Ventanita() {
  return (
    <div className="absolute top-[9%] left-[10%] h-[62%] w-[66%] overflow-hidden rounded-[5px] bg-[#f4f4f5] shadow-[0_6px_18px_rgba(0,0,0,0.35)]">
      <div className="flex h-[14%] items-center justify-end gap-[5%] bg-[#e4e4e7] px-[4%]">
        <span className="h-[2px] w-[7px] bg-zinc-500" />
        <span className="size-[7px] border border-zinc-500" />
        <X className="size-[8px] text-zinc-500" strokeWidth={2.5} />
      </div>
      <div className="space-y-[5%] p-[6%]">
        <div className="h-[6px] w-[50%] rounded-full bg-zinc-400" />
        <div className="h-[4px] w-[88%] rounded-full bg-zinc-300" />
        <div className="h-[4px] w-[78%] rounded-full bg-zinc-300" />
        <div className="h-[4px] w-[84%] rounded-full bg-zinc-300" />
        <div className="mt-[7%] h-[22px] w-[38%] rounded bg-sky-200" />
      </div>
    </div>
  );
}

/**
 * La barra de tareas con la bandeja a la derecha: la flecha de los iconos ocultos, la red,
 * el sonido, winshotx y el reloj (dos rayas, sin cifras).
 */
function BarraDeTareas({ grande = false, children }: { grande?: boolean; children?: ReactNode }) {
  const icono = grande ? "size-[15px]" : "size-[9px]";
  return (
    <div
      className={`absolute inset-x-0 bottom-0 flex items-center justify-end bg-[#202024] ${
        grande ? "h-[30%] gap-[5%] px-[6%]" : "h-[13%] gap-[3%] px-[3%]"
      }`}
    >
      <ChevronUp className={`${icono} text-white/75`} strokeWidth={2.2} />
      <Wifi className={`${icono} text-white/75`} strokeWidth={2.2} />
      <Volume2 className={`${icono} text-white/75`} strokeWidth={2.2} />
      <span
        data-dibujo="winshotx-en-la-bandeja"
        className={`relative flex items-center justify-center rounded-[4px] bg-white/15 ${
          grande ? "size-[30px]" : "size-[15px]"
        }`}
      >
        <Marca className={grande ? "size-[20px]" : "size-[11px]"} />
        {children}
      </span>
      <span className={`flex flex-col items-end ${grande ? "gap-[5px]" : "gap-[2px]"}`}>
        <span className={`rounded-full bg-white/75 ${grande ? "h-[4px] w-[30px]" : "h-[2px] w-[15px]"}`} />
        <span className={`rounded-full bg-white/50 ${grande ? "h-[4px] w-[44px]" : "h-[2px] w-[22px]"}`} />
      </span>
    </div>
  );
}

/** La pantalla de mentira: el escritorio de Windows y una ventana con texto. */
function Pantallita({ children, barra = false }: { children?: ReactNode; barra?: boolean }) {
  return (
    <div
      aria-hidden="true"
      className="relative aspect-[16/10] w-full overflow-hidden rounded-lg"
      style={{ background: ESCRITORIO }}
    >
      <Ventanita />
      {barra && <BarraDeTareas />}
      {children}
    </div>
  );
}

function Cursor({ className = "" }: { className?: string }) {
  return (
    <svg viewBox="0 0 14 20" className={`absolute h-[20px] w-[14px] drop-shadow ${className}`}>
      <path
        d="M1 1 L1 16 L5 12.5 L8 19 L10.5 18 L7.6 11.6 L13 11.6 Z"
        fill="#fff"
        stroke="#111"
        strokeWidth="1.2"
      />
    </svg>
  );
}

/** El recuadro que se arrastra, con lo de fuera oscurecido como en la captura de verdad. */
function Seleccion({ hecha = false }: { hecha?: boolean }) {
  return (
    <div
      className={`absolute rounded-[2px] border-2 border-[#0a9bff] ${hecha ? "" : "border-dashed"}`}
      style={{
        left: "26%",
        top: "24%",
        width: "40%",
        height: "36%",
        boxShadow: hecha ? undefined : "0 0 0 999px rgba(0,0,0,0.45)",
      }}
    />
  );
}

/** Paso 1: se pulsa la tecla, se arrastra la zona, y winshotx sigue en la bandeja. */
export function DibujoAtajos() {
  return (
    <Pantallita barra>
      <Seleccion />
      <Cursor className="top-[57%] left-[64%]" />
    </Pantallita>
  );
}

/**
 * Paso 2: lo que pasa al soltar. Con barra, la barra de la captura debajo de la zona (los
 * mismos iconos que la de verdad: copiar, guardar, editar, anclar, grabar y salir). Al vuelo,
 * la zona ya hecha y su miniatura en la esquina, que es lo que sale al copiarse sola.
 */
export function DibujoAlSoltar({ conBarra }: { conBarra: boolean }) {
  const icono = "size-[11px] text-white/90";
  return (
    <Pantallita>
      <Seleccion hecha={!conBarra} />
      {conBarra ? (
        <>
          <div
            data-dibujo="barra"
            className="absolute flex items-center gap-[6px] rounded-md bg-[#1c1c1f]/95 px-[7px] py-[5px] shadow-[0_4px_12px_rgba(0,0,0,0.4)]"
            style={{ left: "27%", top: "64%" }}
          >
            <Copy className={icono} strokeWidth={2.2} />
            <Download className={icono} strokeWidth={2.2} />
            <Pencil className={icono} strokeWidth={2.2} />
            <Pin className={icono} strokeWidth={2.2} />
            <span className="size-[9px] rounded-full bg-red-500" />
            <X className={icono} strokeWidth={2.2} />
          </div>
          <Cursor className="top-[57%] left-[64%]" />
        </>
      ) : (
        <div
          data-dibujo="miniatura"
          className="absolute right-[4%] bottom-[6%] w-[30%] overflow-hidden rounded-[4px] border-2 border-white bg-[#f4f4f5] shadow-[0_6px_16px_rgba(0,0,0,0.45)]"
        >
          <div className="space-y-[4px] p-[8%]">
            <div className="h-[4px] w-[80%] rounded-full bg-zinc-300" />
            <div className="h-[4px] w-[64%] rounded-full bg-zinc-300" />
            <div className="h-[10px] w-[44%] rounded-sm bg-sky-200" />
          </div>
          <span className="absolute top-[3px] right-[3px] flex size-[14px] items-center justify-center rounded-[3px] bg-[#0a9bff]">
            <Copy className="size-[9px] text-white" strokeWidth={2.6} />
          </span>
        </div>
      )}
    </Pantallita>
  );
}

/** Una tecla del teclado, para leer: estas sí son letras de verdad, a 13 px. */
function Tecla({ children }: { children: ReactNode }) {
  return (
    <span className="rounded-md border-2 border-b-[3px] border-linea-fuerte bg-lienzo px-2.5 py-1 text-[13px] font-semibold whitespace-nowrap text-titulo">
      {children}
    </span>
  );
}

function Abre({ winshotx }: { winshotx: boolean }) {
  const t = useT();
  return (
    <span className="flex items-center gap-2 text-[14px] font-semibold text-titulo">
      <ArrowRight className="size-4 shrink-0 text-tenue" strokeWidth={2} />
      {winshotx ? (
        <>
          <Marca className="size-[18px] shrink-0" x="currentColor" />
          winshotx
        </>
      ) : (
        <>
          <Scissors className="size-4 shrink-0 text-apagado" strokeWidth={2} />
          {t("Herramienta de Recortes")}
        </>
      )}
    </span>
  );
}

/**
 * Paso 3: qué abre cada tecla de captura de Windows. Impr Pant cambia con la elección; Win +
 * Mayús + S no, porque esa no se toca aquí.
 */
export function DibujoImprPant({ deWinshotx }: { deWinshotx: boolean }) {
  const t = useT();
  return (
    // Cada tecla en su línea y lo que abre debajo, siempre: puestos al lado, «Herramienta de
    // Recortes» no cabía y bajaba sola con la flecha delante, distinto en cada idioma.
    <div className="space-y-5 py-1" data-dibujo={deWinshotx ? "impr-pant-winshotx" : "impr-pant-windows"}>
      <div className="flex flex-col items-start gap-2">
        <Tecla>{t("Impr Pant")}</Tecla>
        <Abre winshotx={deWinshotx} />
      </div>
      <div className="flex flex-col items-start gap-2">
        <span className="flex items-center gap-1.5 text-[13px] text-tenue">
          {/* Las teclas tal como las nombra la fila de al lado («Win + Mayús + S» en español,
              «Win + Shift + S» en inglés): sacadas de esa misma frase, no hay dos nombres. */}
          {t("Win + Mayús + S").split(" + ").map((parte, i) => (
            <span key={parte} className="flex items-center gap-1.5">
              {i > 0 && "+"}
              <Tecla>{parte}</Tecla>
            </span>
          ))}
        </span>
        <Abre winshotx={false} />
      </div>
    </div>
  );
}

/**
 * Paso 4: dónde se queda. La esquina de la barra de tareas de cerca, el icono de winshotx con
 * el ratón encima y su menú abierto (rayas, no letras: el menú de verdad lo cuenta la frase).
 */
export function DibujoBandeja() {
  return (
    <div
      aria-hidden="true"
      className="relative aspect-[16/10] w-full overflow-hidden rounded-lg"
      style={{ background: ESCRITORIO }}
    >
      <div
        data-dibujo="menu-de-la-bandeja"
        className="absolute right-[18%] bottom-[34%] w-[46%] space-y-[7px] rounded-md bg-[#f4f4f5] p-[5%] shadow-[0_6px_18px_rgba(0,0,0,0.4)]"
      >
        <div className="flex items-center gap-[6px]">
          <Marca className="size-[12px]" x="#18181b" />
          <span className="h-[5px] w-[40%] rounded-full bg-zinc-500" />
        </div>
        <span className="block h-[4px] w-[72%] rounded-full bg-zinc-300" />
        <span className="block h-[4px] w-[60%] rounded-full bg-zinc-300" />
        <span className="block h-[4px] w-[66%] rounded-full bg-zinc-300" />
      </div>
      <BarraDeTareas grande>
        <Cursor className="top-[45%] left-[45%]" />
      </BarraDeTareas>
    </div>
  );
}
