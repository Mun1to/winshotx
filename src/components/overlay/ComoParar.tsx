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
/**
 * Lo que hace la captura con scroll, con la tecla que la para antes del final.
 *
 * Solo en Linux: ahí la barrita de la captura no se puede sacar de las fotos, así que si no
 * cabe fuera de la zona no sale, y lo que para es esta tecla. Se dice aquí por lo mismo que
 * lo de grabar: es lo último que se ve antes de empezar.
 */
export function ScrollConSuTecla({ tecla }: { tecla: string }) {
  const t = useT();
  const [antes, despues] = t(
    "Recorta lo que se desplaza y winshotx lo bajará solo. Para parar antes del final, pulsa {atajo}.",
  ).split("{atajo}");
  return (
    <>
      {antes}
      <strong className="font-semibold whitespace-nowrap text-white">{tecla}</strong>
      {despues}
    </>
  );
}

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
