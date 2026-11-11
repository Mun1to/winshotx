import { useCallback, useEffect, useRef, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Pencil, Pin, X } from "lucide-react";
import {
  EVENTO_MINIATURAS,
  getSettings,
  miniaturaAnclar,
  miniaturaArrastrar,
  miniaturaCerrar,
  miniaturaCopiar,
  miniaturaEditar,
  miniaturaGuardar,
  miniaturasLista,
  type MiniaturaVista,
  type PilaMiniaturas,
} from "../../lib/ipc";
import { aplicarIdioma, useT } from "../../lib/i18n";
import { aplicarTema } from "../../lib/tema";

/**
 * Las miniaturas de la esquina después de capturar una foto (E2 del plan de CleanShot X).
 *
 * Cada carta es la captura: se arrastra a cualquier app (un chat, un correo, una carpeta)
 * y llega como archivo, y al pasar el ratón salen copiar y guardar en el centro, que es
 * lo que se busca, y editar, anclar y cerrar en las esquinas. Se va sola al rato que diga
 * «Ajustes», salvo mientras el ratón está encima: nadie quiere que se le vaya de debajo
 * del cursor.
 *
 * Las medidas son las de `src-tauri/src/miniatura.rs` (`ANCHO_CARTA`, `ALTO_CARTA`, `HUECO`,
 * `BORDE`): la ventana mide justo lo que miden las cartas, sin hueco ni carta cortada.
 */
export const ANCHO_CARTA = 240;
export const ALTO_CARTA = 150;

/** Cuánto hay que mover el ratón con el botón pulsado para que sea arrastrar y no un clic. */
const UMBRAL_ARRASTRE = 5;
/**
 * Lo mismo empezando encima de «Copiar», «Guardar» o un icono: algo más, para que un clic
 * con el pulso un poco movido siga siendo un clic.
 */
const UMBRAL_DESDE_UN_BOTON = 12;

export function Miniaturas() {
  const [pila, setPila] = useState<PilaMiniaturas>({ lista: [], segundos: 10 });

  useEffect(() => {
    void miniaturasLista()
      .then((p) => p && setPila(p))
      .catch(() => {});
    // Esta ventana nace aparte y no hereda el tema ni el idioma de la de ajustes.
    void getSettings()
      .then((ajustes) => {
        aplicarTema(ajustes.theme);
        aplicarIdioma(ajustes.language);
      })
      .catch(() => {});
    // Con su etiqueta: un `listen` sin destino escucha lo de todas las ventanas (trampa 8).
    const etiqueta = getCurrentWindow().label;
    const quitar = listen<PilaMiniaturas>(EVENTO_MINIATURAS, (e) => setPila(e.payload), {
      target: etiqueta,
    });
    return () => {
      void quitar.then((fn) => fn());
    };
  }, []);

  return (
    <main className="flex h-full flex-col gap-2 bg-lienzo p-2">
      {pila.lista.map((m) => (
        <Carta key={m.id} miniatura={m} segundos={pila.segundos} />
      ))}
    </main>
  );
}

function Carta({ miniatura, segundos }: { miniatura: MiniaturaVista; segundos: number }) {
  const t = useT();
  const { id } = miniatura;
  const [encima, setEncima] = useState(false);
  const [arrastrando, setArrastrando] = useState(false);
  const [aviso, setAviso] = useState<{ texto: string; malo: boolean } | null>(null);
  const inicio = useRef<{ x: number; y: number; enBoton: boolean } | null>(null);
  /** Si esta pulsación ya se ha convertido en arrastre: entonces no es el clic de un botón. */
  const arrastro = useRef(false);

  const cerrar = useCallback(() => void miniaturaCerrar(id).catch(() => {}), [id]);

  // Se va sola, pero no con el ratón encima: se cuenta otra vez entera al salir.
  useEffect(() => {
    if (segundos <= 0 || encima || arrastrando) return;
    const reloj = window.setTimeout(cerrar, segundos * 1000);
    return () => window.clearTimeout(reloj);
  }, [segundos, encima, arrastrando, cerrar]);

  useEffect(() => {
    if (!aviso) return;
    const reloj = window.setTimeout(() => setAviso(null), aviso.malo ? 3000 : 1400);
    return () => window.clearTimeout(reloj);
  }, [aviso]);

  /** Las cuatro acciones terminan igual: un aviso corto encima de la carta. */
  const hacer = async (accion: () => Promise<unknown>, bien: string | null) => {
    try {
      await accion();
      if (bien) setAviso({ texto: bien, malo: false });
    } catch (e) {
      // Los errores llegan de Rust escritos en español, y su texto es la clave.
      setAviso({ texto: String(e), malo: true });
    }
  };

  // Arrastrar empieza al moverse con el botón pulsado, no al pulsar: así un clic en un
  // botón sigue siendo un clic. Y se pide a Rust con el botón todavía abajo, que es lo
  // que espera el arrastre de Windows.
  //
  // **También empezando encima de un botón.** Con el ratón encima, «Copiar» y «Guardar»
  // ocupan el centro de la carta, que es justo por donde la coge todo el mundo, y antes
  // pulsar ahí y moverse no hacía nada (probado en pantalla el 12 de noviembre de 2026).
  const alPulsar = (e: React.PointerEvent) => {
    if (e.button !== 0) return;
    arrastro.current = false;
    const enBoton = (e.target as HTMLElement).closest("button") !== null;
    inicio.current = { x: e.clientX, y: e.clientY, enBoton };
  };
  const alMover = (e: React.PointerEvent) => {
    if (arrastrando && e.buttons === 0) setArrastrando(false);
    const desde = inicio.current;
    if (!desde || (e.buttons & 1) === 0) return;
    const umbral = desde.enBoton ? UMBRAL_DESDE_UN_BOTON : UMBRAL_ARRASTRE;
    if (Math.hypot(e.clientX - desde.x, e.clientY - desde.y) < umbral) return;
    inicio.current = null;
    arrastro.current = true;
    setArrastrando(true);
    void hacer(() => miniaturaArrastrar(id), null);
  };
  // El arrastre de Windows se queda con el soltar, pero por si alguno llega al botón de
  // debajo: un arrastre no es un clic, y no copia ni guarda nada.
  const sinClicTrasArrastrar = (e: React.MouseEvent) => {
    if (!arrastro.current) return;
    arrastro.current = false;
    e.stopPropagation();
    e.preventDefault();
  };
  const alSoltar = () => {
    inicio.current = null;
  };

  return (
    <div
      data-miniatura={id}
      onPointerEnter={() => setEncima(true)}
      onPointerLeave={() => {
        setEncima(false);
        setArrastrando(false);
        inicio.current = null;
      }}
      onPointerDown={alPulsar}
      onPointerMove={alMover}
      onPointerUp={alSoltar}
      onClickCapture={sinClicTrasArrastrar}
      title={t("Arrástrala a cualquier app")}
      style={{ width: ANCHO_CARTA, height: ALTO_CARTA }}
      className="group relative shrink-0 cursor-grab overflow-hidden rounded-md bg-black active:cursor-grabbing"
    >
      <img
        src={convertFileSrc(miniatura.vista)}
        alt=""
        draggable={false}
        className="pointer-events-none size-full object-contain"
      />

      {/* Lo de encima solo con el ratón: en reposo, la carta es la captura y nada más. */}
      <div
        className={`absolute inset-0 flex flex-col bg-black/55 transition-opacity duration-150 ${
          encima && !arrastrando ? "opacity-100" : "pointer-events-none opacity-0"
        }`}
      >
        <div className="flex items-center justify-between p-1.5">
          <Icono icono={X} etiqueta={t("Cerrar")} onClick={cerrar} />
          <span className="flex gap-1">
            <Icono icono={Pencil} etiqueta={t("Editar")} onClick={() => void hacer(() => miniaturaEditar(id), null)} />
            <Icono icono={Pin} etiqueta={t("Anclar")} onClick={() => void hacer(() => miniaturaAnclar(id), null)} />
          </span>
        </div>
        <div className="flex flex-1 items-center justify-center gap-2 pb-6">
          <button
            type="button"
            onClick={() => void hacer(() => miniaturaCopiar(id), "Copiada")}
            className="h-8 rounded-md bg-blue-500 px-3.5 text-[13px] font-semibold text-white transition-colors hover:bg-blue-400"
          >
            {t("Copiar")}
          </button>
          <button
            type="button"
            onClick={() =>
              void hacer(() => miniaturaGuardar(id), miniatura.guardada ? null : "Guardada")
            }
            className="h-8 rounded-md bg-white/90 px-3.5 text-[13px] font-semibold text-neutral-900 transition-colors hover:bg-white"
          >
            {miniatura.guardada ? t("Mostrar") : t("Guardar")}
          </button>
        </div>
      </div>

      {arrastrando && (
        <span className="absolute inset-x-0 bottom-2 text-center text-[12.5px] font-semibold text-white [text-shadow:0_1px_3px_rgb(0_0_0/0.9)]">
          {t("Suéltala en cualquier app")}
        </span>
      )}

      {aviso && (
        <span
          role="status"
          className={`absolute bottom-2 left-1/2 max-w-[92%] -translate-x-1/2 truncate rounded-md px-2 py-1 text-[12px] font-medium text-white shadow-lg ${
            aviso.malo ? "bg-red-500/90" : "bg-emerald-600/90"
          }`}
        >
          {t(aviso.texto)}
        </span>
      )}
    </div>
  );
}

function Icono({
  icono: Dibujo,
  etiqueta,
  onClick,
}: {
  icono: typeof X;
  etiqueta: string;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      title={etiqueta}
      aria-label={etiqueta}
      className="flex size-7 items-center justify-center rounded-md text-white transition-colors hover:bg-white/20"
    >
      <Dibujo className="size-4" />
    </button>
  );
}
