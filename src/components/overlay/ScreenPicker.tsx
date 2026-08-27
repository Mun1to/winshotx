import type { CaptureMode } from "../../lib/types";
import { useT } from "../../lib/i18n";

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
  still: "capturarla entera",
  video: "grabarla entera",
  gif: "grabarla entera en GIF",
};

/**
 * El cartel que dice "esta pantalla es la 2" y que se coge con un clic.
 *
 * Con tres monitores, decir "quiero esa" pedía arrastrar de esquina a esquina de la
 * pantalla correcta. El número es lo que las hace nombrables: se ve en cuál está el ratón
 * y se sabe cuál se va a llevar antes de pulsar.
 */
export function ScreenPicker({ numero, total, modo, ancho, alto }: Props) {
  const t = useT();
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
        <span className="flex items-center gap-2 rounded-full border border-white/10 bg-neutral-900/90 px-4 py-2 text-[13px] text-neutral-200 shadow-xl backdrop-blur-md">
          {total > 1 && (
            // La tecla, porque el numero grande no dice por si solo que se pueda pulsar,
            // y con tres pantallas el raton casi nunca esta en la que se quiere.
            <>
              <kbd className="rounded-[5px] border border-white/15 bg-white/10 px-1.5 py-0.5 text-[11px] leading-none font-medium">
                {numero}
              </kbd>
              <span className="text-neutral-500">{t("o clic para")}</span>
            </>
          )}
          {total === 1 && <span>{t("Clic para")}</span>}
          <span>{QUE_HACE[modo]}</span>
          <span className="text-neutral-500 tabular-nums">
            {ancho} × {alto}
          </span>
        </span>

        {/*
          El `0` va con los numeros de pantalla y significa "todas". Sin decirlo aqui, la
          tecla no la encuentra nadie: no hay nada en pantalla que la sugiera.
          Solo sale con varias pantallas y solo en foto, que es lo unico que sabe hacer.
        */}
        {total > 1 && modo === "still" && (
          <span className="flex items-center gap-2 rounded-full border border-white/10 bg-neutral-900/70 px-3 py-1.5 text-[12px] text-neutral-400 shadow-lg backdrop-blur-md">
            <kbd className="rounded-[5px] border border-white/15 bg-white/10 px-1.5 py-0.5 text-[11px] leading-none font-medium text-neutral-200">
              0
            </kbd>
            <span>para llevarte las {total} de una vez</span>
          </span>
        )}
      </div>
    </div>
  );
}
