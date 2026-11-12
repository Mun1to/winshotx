import type { VolumenDePista } from "../../lib/contrato";
import { useT } from "../../lib/i18n";
import { Slider } from "../ui/Slider";
import { Toggle } from "../ui/Toggle";

interface Props {
  /** El sonido del PC, o null si la grabación no lo trae (entonces no sale su mando). */
  pc: VolumenDePista | null;
  /** Y la voz. */
  voz: VolumenDePista | null;
  /** A otra velocidad el vídeo sale mudo: el volumen no pinta nada y no se ofrece. */
  conVolumen: boolean;
  /** Lo que hay que saber del sonido de esta exportación (lo acelerado va mudo), si algo. */
  aviso?: string;
  onPc: (pista: VolumenDePista) => void;
  onVoz: (pista: VolumenDePista) => void;
}

/**
 * S11 (pistas): donde iba «Sonido», un mando por pista que trae la grabación: «Sonido del PC»
 * y «Tu micrófono», cada uno con su interruptor y, encendido, su volumen. Lo que se toca se
 * oye en la vista previa en el momento y es lo que sale en el archivo.
 */
export function MandosDePistas({ pc, voz, conVolumen, aviso, onPc, onVoz }: Props) {
  const t = useT();
  const mandos: { titulo: string; pista: VolumenDePista; cambiar: (p: VolumenDePista) => void }[] = [];
  if (pc) mandos.push({ titulo: t("Sonido del PC"), pista: pc, cambiar: onPc });
  if (voz) mandos.push({ titulo: t("Tu micrófono"), pista: voz, cambiar: onVoz });
  return (
    <>
      {mandos.map(({ titulo, pista, cambiar }, i) => (
        <div key={titulo} className="space-y-2">
          <Toggle
            checked={pista.encendida}
            onChange={(encendida) => cambiar({ ...pista, encendida })}
            label={titulo}
            hint={i === 0 ? aviso : undefined}
          />
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
