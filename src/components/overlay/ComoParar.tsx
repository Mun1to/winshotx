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

/**
 * Lo mismo cuando no hay tecla que decir: Wayland sin tecla de capturar puesta en el
 * escritorio. Allí no hay barra nunca ni se puede coger Esc, así que se nombra lo que queda:
 * el botón de dejar de compartir del propio escritorio (GNOME, KDE) y, donde no lo hay, la
 * orden. A nadie se le deja sin forma de parar.
 */
export function ScrollSinTecla({ como }: { como: Datos }) {
  const t = useT();
  if (como.escritorio) {
    return (
      <>
        {t(
          "Recorta lo que se desplaza y winshotx lo bajará solo. Para parar antes del final, deja de compartir la pantalla desde el escritorio.",
        )}
      </>
    );
  }
  const [antes, despues] = t(
    "Recorta lo que se desplaza y winshotx lo bajará solo. Para parar antes del final, ejecuta {orden}.",
  ).split("{orden}");
  return (
    <>
      {antes}
      <strong className="font-semibold whitespace-nowrap text-white">
        {como.scrollOrden ?? "winshotx --capture"}
      </strong>
      {despues}
    </>
  );
}

/**
 * Lo que hay que contestarle al escritorio para que la captura con scroll baje sola.
 *
 * En Wayland la rueda la gira el escritorio, y antes pregunta. En la pregunta de GNOME eso
 * viene apagado: quien pulsa «Compartir» sin tocarlo da la pantalla y no el puntero, y la
 * página se queda quieta sin que nada en pantalla diga por qué (aquí no hay barra). Solo
 * sale mientras el escritorio vaya a preguntar. Va en su renglón, y es una frase entera.
 */
export function ScrollPregunta() {
  const t = useT();
  return (
    <span data-scroll-pregunta className="block">
      {t("Cuando el escritorio pregunte, permite la interacción remota: sin ella tendrás que bajarlo tú.")}
    </span>
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
