import type { VolumenDePista } from "../../lib/contrato";
import { useT } from "../../lib/i18n";
import { Slider } from "../ui/Slider";
import { Toggle } from "../ui/Toggle";

/**
 * Por debajo de esto (-40 dBFS de pico) una pista no se oye, ni en la vista previa ni en el
 * archivo. Una voz normal pasa de 0,3, y una app al 10 % en el mezclador de Windows, de 0,03.
 */
export const CASI_MUDA = 0.01;

/**
 * Lo que hay que saber de una pista grabada muda o casi muda, o null si se oye.
 *
 * Sin esto, una grabación sin sonido parecía una vista previa rota: el 14 de noviembre de
 * 2026 el sonido del PC salió a -56 dB porque Brave estaba al 10 % en el mezclador (winshotx
 * graba lo que sale DESPUÉS de esa rueda), y el micro en ceros. Sin dato (las sesiones de
 * antes de la 0.4.6), no se dice nada.
 */
export function avisoDeNivel(pico: number | null | undefined, cual: "pc" | "voz"): string | null {
  if (pico == null || pico >= CASI_MUDA) return null;
  if (cual === "pc") {
    return pico === 0
      ? "No sonó nada en el PC mientras grababas."
      : "Se grabó casi sin sonido: la app que sonaba estaba muy baja en el mezclador de Windows.";
  }
  return pico === 0
    ? "El micrófono no mandó nada: mira que no esté silenciado ni sin permiso en Windows."
    : "Se grabó casi sin sonido: sube el micrófono en los ajustes de sonido de Windows.";
}

interface Props {
  /** El sonido del PC, o null si la grabación no lo trae (entonces no sale su mando). */
  pc: VolumenDePista | null;
  /** Y la voz. */
  voz: VolumenDePista | null;
  /** A otra velocidad el vídeo sale mudo: el volumen no pinta nada y no se ofrece. */
  conVolumen: boolean;
  /** Lo que hay que saber del sonido de esta exportación (lo acelerado va mudo), si algo. */
  aviso?: string;
  /** Lo más alto que sonó cada pista al grabarla, de 0 a 1 (`avisoDeNivel`). */
  picoPc?: number | null;
  picoVoz?: number | null;
  onPc: (pista: VolumenDePista) => void;
  onVoz: (pista: VolumenDePista) => void;
}

/**
 * S11 (pistas): donde iba «Sonido», un mando por pista que trae la grabación: «Sonido del PC»
 * y «Tu micrófono», cada uno con su interruptor y, encendido, su volumen. Lo que se toca se
 * oye en la vista previa en el momento y es lo que sale en el archivo.
 */
export function MandosDePistas({ pc, voz, conVolumen, aviso, picoPc, picoVoz, onPc, onVoz }: Props) {
  const t = useT();
  const mandos: {
    titulo: string;
    pista: VolumenDePista;
    nivel: string | null;
    cambiar: (p: VolumenDePista) => void;
  }[] = [];
  if (pc) mandos.push({ titulo: t("Sonido del PC"), pista: pc, nivel: avisoDeNivel(picoPc, "pc"), cambiar: onPc });
  if (voz) mandos.push({ titulo: t("Tu micrófono"), pista: voz, nivel: avisoDeNivel(picoVoz, "voz"), cambiar: onVoz });
  return (
    <>
      {mandos.map(({ titulo, pista, nivel, cambiar }, i) => (
        <div key={titulo} className="space-y-2">
          <Toggle
            checked={pista.encendida}
            onChange={(encendida) => cambiar({ ...pista, encendida })}
            label={titulo}
            hint={i === 0 ? aviso : undefined}
          />
          {/* Con la pista apagada no hace falta: ya no va a sonar. */}
          {pista.encendida && nivel && (
            <p className="text-[12px] leading-snug text-amber-300">{t(nivel)}</p>
          )}
          {pista.encendida && conVolumen && (
            <Slider
              label={t("Volumen")}
              hint={`${Math.round(pista.volumen * 100)} %`}
              min={0}
              max={200}
              step={5}
              value={Math.round(pista.volumen * 100)}
              onChange={(v) => cambiar({ ...pista, volumen: v / 100 })}
            />
          )}
        </div>
      ))}
    </>
  );
}
