import { useCallback, useEffect, useRef, useState } from "react";
import { emitTo, listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { aPantalla } from "../../lib/pantallas";
import {
  EVENTO_SELECCION,
  SIN_AVISO,
  aFisico,
  etiquetaDeOverlay,
  pantallaDelPunto,
  puntoACss,
  queAvisar,
  tocaOtraPantalla,
  type Aviso,
  type Fase,
  type Punto,
  type SeleccionCompartida,
} from "../../lib/seleccionCompartida";
import type { MonitorInfo, OverlayPayload, Rect } from "../../lib/types";

interface Opciones {
  payload: OverlayPayload | null;
  /** Las pantallas de esta captura, la propia incluida (`monitoresDe`). */
  monitores: MonitorInfo[];
  /** Pixeles fisicos por pixel CSS de ESTA pantalla. */
  escala: number;
  /** Si el congelado ya esta pintado: hasta entonces la escala no es la buena. */
  listo: boolean;
  /** El recorte de este overlay, en SUS CSS y sin recortar a la ventana. */
  selection: Rect | null;
  /** En que anda el gesto de ESTE overlay; `quieta` si no hay gesto aqui. */
  fase: Fase;
  /** El raton del gesto de este overlay, en fisicos; null sin gesto. */
  cursorFis: Punto | null;
  ponerSeleccion: (rect: Rect | null) => void;
}

interface Resultado {
  /** En que anda el gesto de OTRA pantalla; `quieta` si no hay ninguno en marcha. */
  faseRemota: Fase;
  /** El raton de ese gesto, en CSS de esta pantalla, solo si esta encima de ella. */
  cursorRemoto: Punto | null;
}

/**
 * El recorte, uno para todas las pantallas (`lib/seleccionCompartida.ts`).
 *
 * Escucha lo que mandan los demas overlays y lo pone aqui, y cuenta a los demas lo que
 * cambia aqui: al empezar un gesto, al acabarlo, en medio solo si toca otra pantalla (como
 * mucho uno por cuadro) y con cualquier cambio sin gesto, como el teclado. Lo que acaba de
 * llegar no se reenvia, y lo de otra captura o el eco de lo propio se ignora.
 *
 * Vive aparte de `SelectionCanvas.tsx` para no engordarlo: aqui solo hay avisos.
 */
export function useSeleccionCompartida({
  payload,
  monitores,
  escala,
  listo,
  selection,
  fase,
  cursorFis,
  ponerSeleccion,
}: Opciones): Resultado {
  const [faseRemota, setFaseRemota] = useState<Fase>("quieta");
  const [cursorRemoto, setCursorRemoto] = useState<Punto | null>(null);

  // El oyente se registra una vez: lee lo de ahora por referencia.
  const datos = useRef({ payload, monitores, escala, listo, ponerSeleccion });
  datos.current = { payload, monitores, escala, listo, ponerSeleccion };

  /** Lo que los demas ya saben, sea porque lo mande yo o porque me llego. */
  const antes = useRef<Aviso>(SIN_AVISO);
  /** Lo ultimo por mandar en el cuadro que viene, y ese cuadro. */
  const pendiente = useRef<Omit<Aviso, "tocaba"> | null>(null);
  const cuadro = useRef(0);
  /** Lo que llego antes de tener el congelado pintado: se pone en cuanto lo este. */
  const porAplicar = useRef<SeleccionCompartida | null>(null);

  const cancelarCuadro = useCallback(() => {
    if (cuadro.current) cancelAnimationFrame(cuadro.current);
    cuadro.current = 0;
    pendiente.current = null;
  }, []);

  const aplicar = useCallback(
    (carga: SeleccionCompartida) => {
      const { payload: p, escala: s, ponerSeleccion: poner } = datos.current;
      if (!p) return;
      // Otra captura, o el eco de lo que mande yo: no es para aqui.
      if (carga.generacion !== p.generation || carga.emisor === p.monitor.id) return;
      // Lo de la otra pantalla manda: lo que tuviera yo por mandar ya no vale.
      cancelarCuadro();
      antes.current = { rect: carga.rect, fase: "quieta", cursor: null, tocaba: false };
      poner(carga.rect ? aPantalla(carga.rect, p.monitor, s) : null);
      setFaseRemota(carga.fase);
      const c = carga.cursor;
      setCursorRemoto(
        c && carga.fase !== "quieta" && pantallaDelPunto(c, [p.monitor])
          ? puntoACss(c, p.monitor, s)
          : null,
      );
    },
    [cancelarCuadro],
  );

  const enviar = useCallback((estado: Omit<Aviso, "tocaba">) => {
    const { payload: p, monitores: todas } = datos.current;
    if (!p) return;
    const carga: SeleccionCompartida = {
      generacion: p.generation,
      emisor: p.monitor.id,
      rect: estado.rect,
      fase: estado.fase,
      cursor: estado.cursor,
    };
    for (const m of todas) {
      if (m.id !== p.monitor.id) void emitTo(etiquetaDeOverlay(m.id), EVENTO_SELECCION, carga);
    }
    antes.current = {
      ...estado,
      tocaba: tocaOtraPantalla(estado.rect, estado.cursor, p.monitor.id, todas),
    };
  }, []);

  // Cada captura empieza de cero. Lo que haya llegado ya para ESTA captura se guarda.
  const generacion = payload?.generation;
  useEffect(() => {
    antes.current = SIN_AVISO;
    cancelarCuadro();
    setFaseRemota("quieta");
    setCursorRemoto(null);
    if (generacion !== undefined && porAplicar.current?.generacion !== generacion) {
      porAplicar.current = null;
    }
  }, [generacion, cancelarCuadro]);

  // Dirigido a ESTA ventana: sin `target`, `listen` oye todo, tambien lo que va a otra.
  useEffect(() => {
    const etiqueta = getCurrentWindow().label;
    const quitar = listen<SeleccionCompartida>(
      EVENTO_SELECCION,
      ({ payload: carga }) => {
        if (!datos.current.payload || !datos.current.listo) {
          porAplicar.current = carga;
          return;
        }
        aplicar(carga);
      },
      { target: etiqueta },
    );
    return () => {
      void quitar.then((fn) => fn());
      cancelarCuadro();
    };
  }, [aplicar, cancelarCuadro]);

  useEffect(() => {
    if (!listo || !porAplicar.current) return;
    const carga = porAplicar.current;
    porAplicar.current = null;
    aplicar(carga);
  }, [listo, generacion, aplicar]);

  // Lo que cambia aqui, a las demas.
  useEffect(() => {
    if (!payload || !listo || monitores.length < 2) return;
    const yo = payload.monitor;
    const ahora = {
      rect: selection ? aFisico(selection, yo, escala) : null,
      fase,
      cursor: fase === "quieta" ? null : cursorFis,
    };
    const decision = queAvisar(
      ahora,
      antes.current,
      tocaOtraPantalla(ahora.rect, ahora.cursor, yo.id, monitores),
    );
    if (decision === "ya") {
      cancelarCuadro();
      enviar(ahora);
      return;
    }
    // Ya hay uno en camino: que lleve lo ultimo.
    if (cuadro.current) {
      pendiente.current = ahora;
      return;
    }
    if (decision === "nada") return;
    pendiente.current = ahora;
    cuadro.current = requestAnimationFrame(() => {
      cuadro.current = 0;
      const estado = pendiente.current;
      pendiente.current = null;
      if (estado) enviar(estado);
    });
  }, [selection, fase, cursorFis, payload, monitores, escala, listo, enviar, cancelarCuadro]);

  return { faseRemota, cursorRemoto };
}
