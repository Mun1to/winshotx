import type { CaptureMode } from "../../lib/types";

interface Props {
  /** Qué número de pantalla es esta, empezando por 1. */
  numero: number;
  /** Cuántas hay. Con una sola no hace falta numerarla. */
  total: number;
  modo: CaptureMode;
  ancho: number;
  alto: number;
}

const QUE_HACE: Record<CaptureMode, string> = {
  still: "Clic para capturarla entera",
  video: "Clic para grabarla entera",
  gif: "Clic para grabarla entera en GIF",
};

/**
 * El cartel que dice "esta pantalla es la 2" y que se coge con un clic.
 *
 * Con tres monitores, decir "quiero esa" pedía arrastrar de esquina a esquina de la
 * pantalla correcta. El número es lo que las hace nombrables: se ve en cuál está el ratón
 * y se sabe cuál se va a llevar antes de pulsar.
 */
export function ScreenPicker({ numero, total, modo, ancho, alto }: Props) {
  return (
    <div className="pointer-events-none absolute inset-0 flex items-center justify-center">
      {/* Un marco por dentro del borde: por fuera se sale de la pantalla y no se ve. */}
      <div className="absolute inset-[3px] rounded-lg border-2 border-blue-500/70" />

      <div className="flex flex-col items-center gap-3">
        {total > 1 && (
          <span className="flex size-[104px] items-center justify-center rounded-3xl border-2 border-blue-400/50 bg-neutral-900/85 text-[56px] leading-none font-semibold text-white tabular-nums shadow-2xl backdrop-blur-md">
            {numero}
          </span>
        )}
        <span className="rounded-full border border-white/10 bg-neutral-900/90 px-4 py-2 text-[13px] text-neutral-200 shadow-xl backdrop-blur-md">
          {QUE_HACE[modo]}
          <span className="ml-2 text-neutral-500 tabular-nums">
            {ancho} × {alto}
          </span>
        </span>
      </div>
    </div>
  );
}
