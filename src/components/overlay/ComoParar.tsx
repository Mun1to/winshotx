import { fraseDeComoParar } from "../../lib/comoParar";
import { useT } from "../../lib/i18n";
import type { ComoParar as Datos } from "../../lib/types";

/**
 * Cómo se para la grabación que se va a empezar, dicho en la pantalla de elegir zona.
 *
 * Solo sale en Linux, que es donde puede no haber barra de grabar (no se puede dejar fuera
 * del vídeo). Va aquí y no en un aviso al empezar porque esto es lo último que se ve antes
 * de grabar y nunca sale en lo grabado.
 *
 * Debajo de la barra de modos y con la forma del aviso de la captura con scroll: es lo
 * mismo, una frase que dice qué va a pasar. La tecla va en negrita porque es lo que hay que
 * llevarse leído.
 */
export function ComoParar({ como }: { como: Datos }) {
  const t = useT();
  const trozos = t(fraseDeComoParar(como)).split(/(\{atajo\}|\{orden\})/);
  return (
    <div className="pointer-events-none absolute inset-x-0 top-[84px] flex justify-center px-6">
      <div
        data-como-parar
        className="max-w-[760px] rounded-xl border border-white/10 bg-neutral-900/90 px-4 py-2.5 text-center text-[14px] text-balance text-neutral-100 shadow-2xl backdrop-blur-md"
      >
        {trozos.map((trozo, i) =>
          trozo === "{atajo}" || trozo === "{orden}" ? (
            <strong key={i} className="font-semibold whitespace-nowrap text-white">
              {trozo === "{atajo}" ? como.atajo : como.orden}
            </strong>
          ) : (
            trozo
          ),
        )}
      </div>
    </div>
  );
}
