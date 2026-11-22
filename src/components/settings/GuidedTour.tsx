import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import type { CSSProperties } from "react";
import { Coffee, X } from "lucide-react";
import type { ApartadoId } from "./mapaDeAjustes";
import { CAFE } from "../../lib/enlaces";
import { openUrl } from "../../lib/ipc";
import { useT } from "../../lib/i18n";

/**
 * Un paso del tour.
 *
 * - `apartado`: a qué apartado hay que ir antes de enseñarlo, para que lo iluminado esté
 *   en pantalla. Sin esto el foco apuntaría a un hueco vacío. Su icono del riel se enciende
 *   también, para que se sepa dónde se está.
 * - `objetivo`: el valor de un `data-tour="..."` puesto en la interfaz. Si falta, la
 *   tarjeta sale centrada y sin foco.
 */
interface Paso {
  id: string;
  apartado?: ApartadoId;
  objetivo?: string;
  titulo: string;
  texto: string;
  /** Pone el boton de invitar a un cafe dentro de la tarjeta. Solo lo lleva el ultimo. */
  cafe?: boolean;
}

/**
 * El recorrido de los ajustes de la 0.5 (S36): el riel, el dibujo, y las cuatro cosas que
 * no se adivinan mirando.
 *
 * Se cuenta lo que NO se adivina: que los iconos de la izquierda son todos los apartados, que
 * hay un buscador que lleva a cada ajuste, que el dibujo de la derecha cambia con lo que tocas, que se puede capturar con retardo, que hay
 * dos formas de trabajar al soltar el ratón, y que Windows puede ceder sus teclas. Lo que se
 * entiende solo (un interruptor de sonido) no tiene paso: un tour que lee la pantalla en voz
 * alta se cierra a la mitad. Y empieza donde se abre, en Capturar: no te manda adonde ya estás.
 */
const TODOS_LOS_PASOS: Paso[] = [
  {
    id: "riel",
    apartado: "capturar",
    objetivo: "riel",
    titulo: "Todo está en estos ocho",
    texto:
      "Cada icono es un apartado: deja el ratón encima y te dice su nombre. No hay más pantallas ni menús escondidos, y cada apartado cabe entero sin bajar.",
  },
  {
    id: "buscar",
    apartado: "capturar",
    objetivo: "buscar",
    titulo: "Si no sabes dónde está algo",
    texto:
      "Pulsa aquí o Ctrl + F y escribe lo que buscas, como «cursor» o «sonido»: te lleva a su apartado y te ilumina la fila.",
  },
  {
    id: "dibujo",
    apartado: "capturar",
    objetivo: "dibujo",
    titulo: "Así va a salir",
    texto:
      "Este dibujo cambia en cuanto tocas un ajuste, y debajo te lo dice en frases: si apagas la lupa, desaparece de aquí.",
  },
  {
    id: "atajo",
    apartado: "capturar",
    objetivo: "al-pulsar",
    titulo: "La tecla que lo empieza todo",
    texto:
      "Congela la pantalla para que recortes con calma. Y puedes pedirle que espere 3 o 5 segundos: es la única forma de fotografiar un menú abierto, porque al pulsar el atajo se cierra.",
  },
  {
    id: "recorte",
    apartado: "despues",
    objetivo: "despues-de-capturar",
    titulo: "Qué pasa al soltar el ratón",
    texto:
      "Con «Se copia sola» la imagen va al portapapeles y se acabó, cero clics. Con «Sale la barra» eliges cada vez entre copiar, guardar, editar o grabar.",
  },
  {
    id: "teclas",
    apartado: "teclas",
    objetivo: "las-teclas",
    titulo: "Teclas de Windows",
    texto:
      "Windows decide qué aplicación abre los atajos de recorte en Aplicaciones predeterminadas.",
  },
  {
    id: "ayudar",
    apartado: "ayuda",
    objetivo: "acerca",
    titulo: "Ya está: así puedes ayudar",
    texto:
      "winshotx es gratis, sin cuentas y sin anuncios, y lo hago yo solo. Si te ahorra tiempo, un café es lo que lo mantiene en pie. Y si no, una estrella en GitHub o contar un fallo ayudan igual.",
    cafe: true,
  },
];

/** Los mismos en Linux, sin la parada de las teclas de Windows: el riel tiene un icono menos. */
const PASOS_SIN_TECLAS = TODOS_LOS_PASOS.filter((p) => p.apartado !== "teclas").map((p) =>
  p.id === "riel" ? { ...p, titulo: "Todo está en estos siete" } : p,
);

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
/** Lo que el aro se abre hacia fuera de lo que ilumina. */
const HOLGURA_X = 8;
const HOLGURA_Y = 4;
/** Al lado, la tarjeta se estrecha hasta esto antes de pisar lo que explica. */
const ANCHO_MINIMO_TARJETA = 280;
/** Contra el borde de la ventana basta con menos aire que entre la tarjeta y el aro. */
const BORDE = 8;

const entre = (valor: number, minimo: number, maximo: number) =>
  Math.min(Math.max(valor, minimo), Math.max(minimo, maximo));

export interface SitioTarjeta {
  top: number;
  left: number;
  width: number;
}

/** Cuánto de `b` tapa la tarjeta puesta en `s`, en px². */
function tapa(s: SitioTarjeta, alto: number, b: Marco | null) {
  if (!b) return 0;
  const x = Math.min(s.left + s.width, b.left + b.width) - Math.max(s.left, b.left);
  const y = Math.min(s.top + alto, b.top + b.height) - Math.max(s.top, b.top);
  return x > 0 && y > 0 ? x * y : 0;
}

/**
 * Dónde va la tarjeta del tour. Nunca puede tapar aquello de lo que está hablando: ese es el
 * único trabajo que tiene el foco. Se prueban tres sitios en orden: debajo, encima y al lado.
 * El tercero hace falta de verdad: desde la 0.5 la ventana mide lo que su apartado, y un
 * grupo alto no deja hueco ni debajo ni encima.
 *
 * Al lado va donde haya más sitio, y si no cabe entera, se estrecha (hasta 280) y se arrima
 * al borde: antes caía a la izquierda, encima de lo que explicaba; a la derecha queda sobre
 * el dibujo, que está apagado bajo el velo. Y si tampoco así (en inglés, la del café es alta
 * y su grupo ocupa todo el ancho), gana el sitio que menos tapa del aro y del icono del riel:
 * al lado, o pegada arriba o abajo del todo.
 */
export function colocarTarjeta(
  marco: Marco | null,
  pestana: Marco | null,
  altoTarjeta: number,
  ancho: number,
  alto: number,
): SitioTarjeta {
  if (!marco) {
    return { top: alto / 2 - altoTarjeta / 2, left: ancho / 2 - ANCHO_TARJETA / 2, width: ANCHO_TARJETA };
  }
  const centrada = entre(
    marco.left + marco.width / 2 - ANCHO_TARJETA / 2,
    HUECO,
    ancho - ANCHO_TARJETA - HUECO,
  );
  const debajo = marco.top + marco.height + HUECO;
  const encima = marco.top - altoTarjeta - HUECO;
  if (debajo + altoTarjeta + HUECO <= alto) return { top: debajo, left: centrada, width: ANCHO_TARJETA };
  if (encima >= HUECO) return { top: encima, left: centrada, width: ANCHO_TARJETA };

  const derecha = marco.left + marco.width + HUECO;
  const sitioDerecha = ancho - BORDE - derecha;
  const sitioIzquierda = marco.left - HUECO - BORDE;
  const porLaDerecha = sitioDerecha >= sitioIzquierda;
  const width = entre(porLaDerecha ? sitioDerecha : sitioIzquierda, ANCHO_MINIMO_TARJETA, ANCHO_TARJETA);
  const alLado: SitioTarjeta = {
    top: entre(marco.top + marco.height / 2 - altoTarjeta / 2, HUECO, alto - altoTarjeta - HUECO),
    left: porLaDerecha
      ? Math.min(derecha, ancho - width - BORDE)
      : Math.max(BORDE, marco.left - HUECO - width),
    width,
  };
  const candidatos: SitioTarjeta[] = [
    alLado,
    { top: BORDE, left: centrada, width: ANCHO_TARJETA },
    { top: alto - altoTarjeta - BORDE, left: centrada, width: ANCHO_TARJETA },
  ];
  const cuanto = (s: SitioTarjeta) => tapa(s, altoTarjeta, marco) + tapa(s, altoTarjeta, pestana);
  return candidatos.reduce((mejor, s) => (cuanto(s) < cuanto(mejor) ? s : mejor));
}

interface Props {
  onNavegar: (apartado: ApartadoId) => void;
  onCerrar: () => void;
  /** S21 (Linux): sin la parada de las teclas de Windows, que ahí no existen. */
  sinTeclasDeWindows?: boolean;
}

/**
 * Un agujero del velo: lo que va en negro dentro de la máscara se ve sin oscurecer.
 *
 * La geometría va en `style` y no en atributos porque así es CSS, y el CSS se puede animar:
 * con atributos el agujero saltaba de golpe mientras el aro de color se deslizaba.
 */
function Agujero({
  marco,
  radio,
  holgura = 0,
  nombre,
}: {
  marco: Marco;
  radio: number;
  holgura?: number;
  nombre: string;
}) {
  return (
    <rect
      data-agujero={nombre}
      rx={radio}
      fill="black"
      className="motion-safe:transition-[x,y,width,height] motion-safe:duration-200"
      style={
        {
          x: marco.left - holgura,
          y: marco.top - holgura,
          width: marco.width + holgura * 2,
          height: marco.height + holgura * 2,
        } as CSSProperties
      }
    />
  );
}

export function GuidedTour({ onNavegar, onCerrar, sinTeclasDeWindows = false }: Props) {
  const PASOS = sinTeclasDeWindows ? PASOS_SIN_TECLAS : TODOS_LOS_PASOS;
  const t = useT();
  const [indice, setIndice] = useState(0);
  const [marco, setMarco] = useState<Marco | null>(null);
  // S24: el apartado de la parada también se ve encendido (desde la 0.5, su icono del riel).
  // Sin esto se quedaba debajo del velo y no se sabía en qué apartado estaba uno.
  const [pestana, setPestana] = useState<Marco | null>(null);
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
    if (paso.apartado) onNavegar(paso.apartado);
  }, [paso, onNavegar]);

  const medir = useCallback(() => {
    // Por su nombre en el riel y no por el que está elegido: esto puede medirse un instante
    // antes de que el apartado cambie, y los iconos no se mueven de sitio. En la parada del
    // riel no hace falta: ya está iluminado entero.
    const boton =
      paso.apartado && paso.objetivo !== "riel"
        ? document.querySelector<HTMLElement>(`[data-riel] [data-vista="${paso.apartado}"]`)
        : null;
    if (boton) {
      const r = boton.getBoundingClientRect();
      setPestana({ top: r.top, left: r.left, width: r.width, height: r.height });
    } else {
      setPestana(null);
    }
    if (!paso.objetivo) {
      setMarco(null);
      return;
    }
    const el = document.querySelector<HTMLElement>(`[data-tour="${paso.objetivo}"]`);
    if (!el) {
      setMarco(null);
      return;
    }
    // Desde la 0.5 los grupos van sueltos, sin tarjeta ni relleno: medido al justo, el aro
    // pisaba la primera letra de «Los atajos» y el «Ctrl+F» del buscador. Se abre un poco
    // hacia fuera, sin salirse de la ventana.
    const r = el.getBoundingClientRect();
    const left = Math.max(2, r.left - HOLGURA_X);
    const top = Math.max(2, r.top - HOLGURA_Y);
    const right = Math.min(window.innerWidth - 2, r.right + HOLGURA_X);
    const bottom = Math.min(window.innerHeight - 2, r.bottom + HOLGURA_Y);
    setMarco({ top, left, width: right - left, height: bottom - top });
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
  }, [onCerrar, PASOS]);

  const sitio = colocarTarjeta(marco, pestana, altoTarjeta, window.innerWidth, window.innerHeight);

  // Todo dentro de UN contenedor fijo, y no tres sueltos: asi el tour entero forma su
  // propia capa por encima de la ventana, se traga los clics de paso, y no hay que ir
  // repartiendo z-index por los tres trozos y confiar en que ganen.
  return (
    <div className="fixed inset-0 z-[9999]">
      {/* El velo es un SVG con agujeros y no una sombra de 9999 px alrededor de lo
          iluminado: con la sombra solo cabía UN agujero, y desde S24 hay dos (lo que se
          explica y la pestaña de su sección). Cada agujero se desliza de un sitio a otro,
          salvo para quien pide menos movimiento en Windows: ahí salta y ya está. */}
      <svg aria-hidden="true" className="pointer-events-none fixed inset-0 size-full" data-tour-velo>
        <defs>
          <mask id="tour-agujeros">
            <rect width="100%" height="100%" fill="white" />
            {marco && <Agujero marco={marco} radio={12} nombre="objetivo" />}
            {pestana && <Agujero marco={pestana} radio={8} holgura={3} nombre="pestana" />}
          </mask>
        </defs>
        <rect width="100%" height="100%" fill="rgba(0,0,0,0.72)" mask="url(#tour-agujeros)" />
      </svg>
      {marco && (
        <div
          aria-hidden="true"
          className="pointer-events-none rounded-xl ring-2 ring-marca motion-safe:transition-[top,left,width,height] motion-safe:duration-200"
          style={{ position: "fixed", ...marco }}
        />
      )}

      <div
        ref={tarjeta}
        role="dialog"
        aria-label={t(paso.titulo)}
        className="fixed rounded-xl border border-linea-fuerte bg-flotante p-4 shadow-2xl"
        style={sitio}
      >
        <button
          type="button"
          onClick={onCerrar}
          aria-label={t("Cerrar el tour")}
          className="absolute end-2.5 top-2.5 rounded-md p-1 text-apagado transition-colors hover:bg-realce hover:text-texto"
        >
          <X className="size-4" />
        </button>

        <h2 className="pe-6 text-[15px] font-semibold text-titulo">{t(paso.titulo)}</h2>
        <p className="mt-1.5 text-[13.5px] leading-relaxed text-texto">{t(paso.texto)}</p>

        {paso.cafe && (
          <button
            type="button"
            onClick={() => void openUrl(CAFE)}
            className="mt-3 flex w-full items-center justify-center gap-2 rounded-lg border border-linea bg-realce py-2 text-[13px] font-semibold text-titulo transition-colors hover:bg-activo"
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
                className="rounded-lg px-2.5 py-1.5 text-[13px] font-medium text-apagado transition-colors hover:bg-realce hover:text-titulo"
              >
                {t("Atrás")}
              </button>
            )}
            <button
              type="button"
              autoFocus
              onClick={() => (ultimo ? onCerrar() : setIndice((i) => i + 1))}
              className="rounded-lg bg-marca px-3 py-1.5 text-[13px] font-semibold text-white transition-colors hover:brightness-110"
            >
              {ultimo ? t("Listo") : t("Siguiente")}
            </button>
          </span>
        </div>
      </div>
    </div>
  );
}
