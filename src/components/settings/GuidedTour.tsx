import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import type { CSSProperties } from "react";
import { Coffee, X } from "lucide-react";
import type { SeccionId } from "./SettingsHeader";
import { CAFE } from "../../lib/enlaces";
import { openUrl } from "../../lib/ipc";
import { useT } from "../../lib/i18n";

/**
 * Un paso del tour.
 *
 * - `seccion`: a qué pestaña hay que ir antes de enseñarlo, para que lo iluminado esté
 *   en pantalla. Sin esto el foco apuntaría a un hueco vacío.
 * - `objetivo`: el valor de un `data-tour="..."` puesto en la interfaz. Si falta, la
 *   tarjeta sale centrada y sin foco.
 */
interface Paso {
  id: string;
  seccion?: SeccionId;
  objetivo?: string;
  titulo: string;
  texto: string;
  /** Pone el boton de invitar a un cafe dentro de la tarjeta. Solo lo lleva el ultimo. */
  cafe?: boolean;
}

/**
 * El recorrido: una parada por sección, y dos en Capturar, que es la que se usa a diario.
 *
 * Se cuenta lo que NO se adivina mirando: que hay cuatro pestañas y ninguna pantalla
 * escondida, que se puede capturar con retardo, que hay dos formas de trabajar al soltar
 * el ratón, y que Windows puede ceder sus teclas. Lo que se entiende solo (un interruptor
 * de sonido) no tiene paso: un tour que lee la pantalla en voz alta se cierra a la mitad.
 */
const PASOS: Paso[] = [
  {
    id: "secciones",
    seccion: "capturar",
    objetivo: "secciones",
    titulo: "Todo está en estas cuatro",
    texto:
      "No hay más pantallas ni menús escondidos: lo que se puede cambiar está repartido aquí, y cada una cabe entera sin bajar.",
  },
  {
    id: "atajo",
    seccion: "capturar",
    objetivo: "al-pulsar",
    titulo: "La tecla que lo empieza todo",
    texto:
      "Congela la pantalla para que recortes con calma. Y puedes pedirle que espere 3 o 5 segundos: es la única forma de fotografiar un menú abierto, porque al pulsar el atajo se cierra.",
  },
  {
    id: "recorte",
    seccion: "capturar",
    objetivo: "la-captura",
    titulo: "Qué pasa al soltar el ratón",
    texto:
      "Con «Se copia sola» la imagen va al portapapeles y se acabó, cero clics. Con «Sale la barra» eliges cada vez entre copiar, guardar, editar o grabar.",
  },
  {
    id: "grabar",
    seccion: "grabar",
    objetivo: "como-se-graba",
    titulo: "Lo mismo, pero en movimiento",
    texto:
      "El recorte se graba en GIF o en vídeo. Tiene su propio atajo, y el mismo que empieza la grabación es el que la termina.",
  },
  {
    id: "teclas",
    seccion: "teclas",
    objetivo: "las-teclas",
    titulo: "Quitarle las teclas a Windows",
    texto:
      "Impr Pant es gratis. Win+Mayús+S cuesta perder Win+S, la búsqueda, y la fila te lo dice antes de que pulses nada.",
  },
  {
    id: "archivos",
    seccion: "app",
    objetivo: "archivos",
    titulo: "Dónde acaba lo que capturas",
    texto:
      "La carpeta a la que van las capturas y los vídeos que guardas. El nombre lo pone winshotx con la fecha y la hora, y nunca pisa uno que ya exista.",
  },
  {
    id: "ayudar",
    seccion: "app",
    objetivo: "acerca",
    titulo: "Ya está: así puedes ayudar",
    texto:
      "winshotx es gratis, sin cuentas y sin anuncios, y lo hago yo solo. Si te ahorra tiempo, un café es lo que lo mantiene en pie. Y si no, una estrella en GitHub o contar un fallo ayudan igual.",
    cafe: true,
  },
];

interface Marco {
  top: number;
  left: number;
  width: number;
  height: number;
}

const ANCHO_TARJETA = 330;
/** Alto de partida, hasta que se mide el de verdad en el primer pintado. */
const ALTO_INICIAL = 180;
const HUECO = 14;

const entre = (valor: number, minimo: number, maximo: number) =>
  Math.min(Math.max(valor, minimo), Math.max(minimo, maximo));

interface Props {
  onNavegar: (seccion: SeccionId) => void;
  onCerrar: () => void;
}

export function GuidedTour({ onNavegar, onCerrar }: Props) {
  const t = useT();
  const [indice, setIndice] = useState(0);
  const [marco, setMarco] = useState<Marco | null>(null);
  // El alto de la tarjeta se MIDE, no se estima: los textos no miden todos lo mismo y de
  // ese numero depende si la tarjeta cabe debajo de lo iluminado o hay que sacarla al
  // lado. Con un valor fijo, un paso de texto largo se salia por abajo de la ventana.
  const tarjeta = useRef<HTMLDivElement>(null);
  const [altoTarjeta, setAltoTarjeta] = useState(ALTO_INICIAL);
  const paso = PASOS[indice];
  const ultimo = indice === PASOS.length - 1;

  // Cambiar de pestaña ANTES de medir: al revés se mide lo de la pestaña anterior y el
  // foco aparece un instante sobre lo que no toca.
  useEffect(() => {
    if (paso.seccion) onNavegar(paso.seccion);
  }, [paso, onNavegar]);

  const medir = useCallback(() => {
    if (!paso.objetivo) {
      setMarco(null);
      return;
    }
    const el = document.querySelector<HTMLElement>(`[data-tour="${paso.objetivo}"]`);
    if (!el) {
      setMarco(null);
      return;
    }
    const r = el.getBoundingClientRect();
    setMarco({ top: r.top, left: r.left, width: r.width, height: r.height });
  }, [paso]);

  // Se mide EN EL ACTO, dentro del mismo paso de layout: cuando esto corre, el DOM del
  // paso nuevo ya está puesto, así que casi siempre el objetivo está ahí y no hay que
  // esperar a nada. Dejarlo todo en manos de `requestAnimationFrame` era lo que hacía que
  // el foco se quedara clavado donde el paso anterior.
  //
  // El reintento sigue existiendo, pero solo para el caso que lo pedía: cambiar de pestaña
  // monta una sección que no estaba, y ahí el objetivo tarda un fotograma en aparecer.
  useLayoutEffect(() => {
    if (!paso.objetivo) {
      setMarco(null);
      return;
    }
    if (document.querySelector(`[data-tour="${paso.objetivo}"]`)) {
      medir();
      return;
    }
    let raf = 0;
    let intentos = 0;
    const probar = () => {
      if (!document.querySelector(`[data-tour="${paso.objetivo}"]`) && intentos < 30) {
        intentos += 1;
        raf = requestAnimationFrame(probar);
        return;
      }
      medir();
    };
    raf = requestAnimationFrame(probar);
    return () => cancelAnimationFrame(raf);
  }, [paso, medir]);

  // Medir despues de pintar y antes de que se vea: en el mismo fotograma en el que la
  // tarjeta cambia de texto, para que no llegue a ensennarse colocada con el alto viejo.
  useLayoutEffect(() => {
    const h = tarjeta.current?.getBoundingClientRect().height;
    if (h) setAltoTarjeta((previo) => (Math.abs(previo - h) < 1 ? previo : h));
  });

  useEffect(() => {
    const alCambiar = () => medir();
    window.addEventListener("resize", alCambiar);
    return () => window.removeEventListener("resize", alCambiar);
  }, [medir]);

  useEffect(() => {
    const alPulsar = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        onCerrar();
        return;
      }
      if (e.key === "ArrowRight") {
        setIndice((i) => Math.min(PASOS.length - 1, i + 1));
      }
      if (e.key === "ArrowLeft") {
        setIndice((i) => Math.max(0, i - 1));
      }
    };
    window.addEventListener("keydown", alPulsar);
    return () => window.removeEventListener("keydown", alPulsar);
  }, [onCerrar]);

  // La tarjeta nunca puede tapar aquello de lo que esta hablando: ese es el unico trabajo
  // que tiene el foco. Se prueban tres sitios en orden: debajo, encima y al lado. El
  // tercero hace falta de verdad: un bloque como "Al pulsar el atajo" mide 270 px de alto
  // en una ventana de 470, asi que ni debajo ni encima queda hueco, y sin esta salida la
  // tarjeta acababa encima de lo que venia a ensennar.
  const alto = window.innerHeight;
  const ancho = window.innerWidth;
  let sitio: CSSProperties;
  if (!marco) {
    sitio = { top: alto / 2 - altoTarjeta / 2, left: ancho / 2 - ANCHO_TARJETA / 2 };
  } else {
    const centrada = entre(
      marco.left + marco.width / 2 - ANCHO_TARJETA / 2,
      HUECO,
      ancho - ANCHO_TARJETA - HUECO,
    );
    const debajo = marco.top + marco.height + HUECO;
    const encima = marco.top - altoTarjeta - HUECO;
    if (debajo + altoTarjeta + HUECO <= alto) {
      sitio = { top: debajo, left: centrada };
    } else if (encima >= HUECO) {
      sitio = { top: encima, left: centrada };
    } else {
      const derecha = marco.left + marco.width + HUECO;
      sitio = {
        top: entre(
          marco.top + marco.height / 2 - altoTarjeta / 2,
          HUECO,
          alto - altoTarjeta - HUECO,
        ),
        left:
          derecha + ANCHO_TARJETA + HUECO <= ancho
            ? derecha
            : Math.max(HUECO, marco.left - ANCHO_TARJETA - HUECO),
      };
    }
  }

  // Todo dentro de UN contenedor fijo, y no tres sueltos: asi el tour entero forma su
  // propia capa por encima de la ventana, se traga los clics de paso, y no hay que ir
  // repartiendo z-index por los tres trozos y confiar en que ganen.
  return (
    <div className="fixed inset-0 z-[9999]">
      {marco ? (
        // La sombra de 9999px es la que oscurece la ventana entera, y el hueco es este
        // rectángulo. Sale más barato y más exacto que recortar cuatro tiras alrededor.
        // Se anima solo la geometría: metiendo la sombra en la transición, el oscurecido
        // entraba desde cero y en una foto salía la ventana sin oscurecer.
        <div
          aria-hidden="true"
          // El foco se desliza de un sitio a otro, salvo para quien pide menos
          // movimiento en Windows: ahi salta y ya esta. Solo se anima la geometria;
          // metiendo la sombra dentro, el oscurecido entraba desde cero.
          className="pointer-events-none rounded-xl ring-2 ring-marca motion-safe:transition-[top,left,width,height] motion-safe:duration-200"
          style={{
            position: "fixed",
            ...marco,
            boxShadow: "0 0 0 9999px rgba(0,0,0,0.72)",
          }}
        />
      ) : (
        <div aria-hidden="true" className="pointer-events-none fixed inset-0 bg-black/70" />
      )}

      <div
        ref={tarjeta}
        role="dialog"
        aria-label={t(paso.titulo)}
        className="fixed rounded-xl border border-linea-fuerte bg-flotante p-4 shadow-2xl"
        style={{ ...sitio, width: ANCHO_TARJETA }}
      >
        <button
          type="button"
          onClick={onCerrar}
          aria-label={t("Cerrar el tour")}
          className="absolute end-2.5 top-2.5 rounded-md p-1 text-tenue transition-colors hover:bg-realce hover:text-texto"
        >
          <X className="size-3.5" />
        </button>

        <h2 className="pe-6 text-[14.5px] font-semibold text-titulo">{t(paso.titulo)}</h2>
        <p className="mt-1.5 text-[12.5px] leading-relaxed text-apagado">{t(paso.texto)}</p>

        {paso.cafe && (
          <button
            type="button"
            onClick={() => void openUrl(CAFE)}
            className="mt-3 flex w-full items-center justify-center gap-2 rounded-lg border border-linea-fuerte py-2 text-[12.5px] font-medium text-suave transition-colors hover:bg-realce hover:text-titulo"
          >
            <Coffee className="size-4" />
            {t("Invítame a un café")}
          </button>
        )}

        <div className="mt-3.5 flex items-center justify-between">
          <span className="flex items-center gap-1.5" aria-hidden="true">
            {PASOS.map((p, i) => (
              <span
                key={p.id}
                className={`h-1.5 rounded-full motion-safe:transition-all motion-safe:duration-200 ${
                  i === indice ? "w-4 bg-marca" : "w-1.5 bg-activo"
                }`}
              />
            ))}
          </span>

          <span className="flex items-center gap-2">
            {indice > 0 && (
              <button
                type="button"
                onClick={() => setIndice((i) => i - 1)}
                className="rounded-lg px-2.5 py-1.5 text-[12px] text-apagado transition-colors hover:bg-realce hover:text-titulo"
              >
                {t("Atrás")}
              </button>
            )}
            <button
              type="button"
              autoFocus
              onClick={() => (ultimo ? onCerrar() : setIndice((i) => i + 1))}
              className="rounded-lg bg-blue-600 px-3 py-1.5 text-[12px] font-medium text-white transition-colors hover:bg-blue-500"
            >
              {ultimo ? t("Listo") : t("Siguiente")}
            </button>
          </span>
        </div>
      </div>
    </div>
  );
}
