import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  cacheStats,
  cambiarTeclasEnElEscritorio,
  capturaModerna,
  clearCache,
  getSettings,
  justUpdated,
  listScreens,
  printScreenState,
  quitApp,
  removeSnippingTool,
  replayStatus,
  restartShell,
  setSettings,
  shortcutStatus,
  sonidoDisponible,
  usePrintScreen,
  useWinShiftS,
  // S2 (grabación)
  listCameras,
  type CamaraConectada,
} from "../../lib/ipc";
import { useT } from "../../lib/i18n";
import { Historial } from "../historial/Historial";
import {
  EVENTS,
  type CacheStats,
  type PrintScreenState,
  type ReplayStatus,
  type Screen,
  type Settings,
  type ShortcutStatus,
  type SonidoDisponible,
} from "../../lib/types";
import { useSobra } from "../../lib/ventanaFija";
import { AvisoActualizacion, useActualizacion } from "./Actualizacion";
import {
  ApartadoApp,
  ApartadoAyuda,
  ApartadoCapturar,
  ApartadoDespues,
  ApartadoGrabar,
  ApartadoTeclas,
  ApartadoUltimos,
  type DatosAjustes,
} from "./Apartados";
import { apartado as datosDe, type ApartadoId } from "./mapaDeAjustes";
import { Cabecera } from "./Cabecera";
import { GuidedTour } from "./GuidedTour";
import { Paleta } from "./Paleta";
import { FilaEncendida } from "./Section";
import { BarraRapida } from "./BarraRapida";
import { Riel } from "./Riel";
import { VistaPrevia } from "./VistaPrevia";
// S21 (Linux): lo que solo es de Windows se esconde, y en Wayland las teclas las pone el escritorio.
import { usePlataforma, useTeclasDelEscritorio } from "../../lib/plataforma";

/** La inyecta Vite desde package.json: escribirla a mano acababa en cuatro copias. */
const VERSION = __VERSION__;

/** Las filas que en Linux no salen, para que el buscador tampoco las ofrezca. */
const SOLO_EN_WINDOWS = new Set(["capIconos", "tecImpr", "tecWinS", "tecRecortes", "tecRecortesWindows"]);

const APARTADO: Record<ApartadoId, (d: DatosAjustes) => React.ReactNode> = {
  capturar: ApartadoCapturar,
  despues: ApartadoDespues,
  grabar: ApartadoGrabar,
  ultimos: ApartadoUltimos,
  // El historial no es una lista de filas: va aparte, a todo el ancho.
  historial: () => null,
  teclas: ApartadoTeclas,
  app: ApartadoApp,
  ayuda: ApartadoAyuda,
};

interface SettingsAppProps {
  onVerBienvenida: () => void;
  /** Arranca el tour nada mas montarse: es lo que pasa al terminar la bienvenida. */
  arrancarTour?: boolean;
}

/**
 * La ventana de ajustes (S36, la variación 9 que eligió Munito el 15-nov-2026).
 *
 * Arriba la cabecera (el nombre y la versión, el buscador y salir); a la izquierda el riel
 * con un icono por apartado; en medio el apartado elegido, con su título, su frase y sus
 * filas; y a la derecha, «Así va a salir», el dibujo de lo que va a pasar con lo que está
 * elegido. Un apartado a la vez, y cada uno cabe entero: la ventana mide lo que mide él.
 *
 * Aquí vive el estado y lo que se habla con Rust; las filas están en `Apartados.tsx`.
 */
export function SettingsApp({ onVerBienvenida, arrancarTour = false }: SettingsAppProps) {
  const t = useT();
  // Que apartado se esta viendo. Es estado de la pantalla y no un ajuste: no tiene sentido
  // guardarlo en disco ni que sobreviva a cerrar la ventana.
  const [seccion, setSeccion] = useState<ApartadoId>("capturar");
  // Si esta ventana se ha abierto sola porque se acaba de actualizar, hay que decir por que:
  // el aviso de la cabecera lo cuenta, y se abre en «Ayuda y winshotx», donde esta la version.
  const [recienActualizado, setRecienActualizado] = useState(false);
  const [tour, setTour] = useState(arrancarTour);
  const [settings, setLocal] = useState<Settings | null>(null);
  const [shortcuts, setShortcuts] = useState<ShortcutStatus>({
    capture: true,
    edit: true,
    record: true,
    replay: false,
    printScreen: false,
    winShiftS: false,
  });
  const [pantallas, setPantallas] = useState<Screen[]>([]);
  const [replay, setReplay] = useState<ReplayStatus>({
    running: false,
    seconds: 0,
    screen: 0,
    screenLabel: "",
    bytes: 0,
    bytesPerSecond: 0,
    width: 0,
    height: 0,
    bufferedMs: 0,
  });
  const [cache, setCache] = useState<CacheStats>({ bytes: 0, sessions: 0 });
  const [imprPant, setImprPant] = useState<PrintScreenState | null>(null);
  /** null = sin tocar · "confirmar" = esperando el segundo clic · el resto, el resultado. */
  const [recortes, setRecortes] = useState<string | null>(null);
  /** Mientras se reinicia el Explorador, que son un par de segundos largos. */
  const [aplicando, setAplicando] = useState(false);
  const [error, setError] = useState<string | null>(null);
  /**
   * Si las fotos van por la captura moderna. Arranca en `true` porque es lo normal: si
   * arrancara en `false`, el aviso saldria un instante en la pantalla de todo el mundo
   * antes de que Rust conteste.
   */
  const [modernaOk, setModernaOk] = useState(true);
  // S2 (grabación): las cámaras conectadas, para la fila de la cámara.
  const [camaras, setCamaras] = useState<CamaraConectada[]>([]);
  // S21 (Linux): dónde corre, y en Wayland las teclas que ha puesto el escritorio.
  const donde = usePlataforma();
  const enLinux = donde?.sistema === "linux";
  const enWayland = donde?.wayland ?? false;
  const teclasDelEscritorio = useTeclasDelEscritorio(enWayland);
  // Si «Cambiar» no abre nada (Hyprland, o un escritorio que winshotx no conoce), queda lo
  // mismo que donde no hay portal: poner la tecla a mano con la orden.
  const [sinVentanaDeTeclas, setSinVentanaDeTeclas] = useState(false);
  const teclasAMano = enWayland && Boolean(teclasDelEscritorio?.aMano || sinVentanaDeTeclas);
  const ordenDeLaApp = teclasDelEscritorio?.orden ?? "winshotx";
  const cambiarTeclas = useCallback(() => {
    void cambiarTeclasEnElEscritorio().catch(() => setSinVentanaDeTeclas(true));
  }, []);
  // S45: el de la tecla de rescatar va aparte, porque esa tecla se puede haber rechazado sola
  // y entonces «Cambiar» es volver a pedírsela al escritorio.
  const cambiarTeclaDeRescate = useCallback(() => {
    void cambiarTeclasEnElEscritorio(true).catch(() => setSinVentanaDeTeclas(true));
  }, []);
  // S44 (Linux): qué sonido se puede grabar aquí. Se vuelve a mirar al volver a la ventana,
  // por si entretanto se ha enchufado un micrófono.
  const [sonido, setSonido] = useState<SonidoDisponible | null>(null);
  useEffect(() => {
    if (!enLinux) return;
    let vivo = true;
    const mirar = () =>
      void sonidoDisponible()
        .then((hay) => vivo && setSonido(hay ?? null))
        .catch(() => undefined);
    mirar();
    window.addEventListener("focus", mirar);
    return () => {
      vivo = false;
      window.removeEventListener("focus", mirar);
    };
  }, [enLinux]);
  const actualizacion = useActualizacion(VERSION, recienActualizado);
  // El buscador: si la paleta está abierta, y la fila a la que se ha saltado, que se ilumina.
  const [paleta, setPaleta] = useState(false);
  const [encendida, setEncendida] = useState<string | null>(null);
  const enTour = useRef(tour);
  enTour.current = tour;

  // Ctrl + F abre el buscador desde cualquier sitio de la ventana, como en un navegador.
  // Con el tour delante no: el tour tapa la ventana y el buscador saldría debajo.
  useEffect(() => {
    const tecla = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && !e.altKey && e.key.toLowerCase() === "f" && !enTour.current) {
        e.preventDefault();
        setPaleta(true);
      }
    };
    window.addEventListener("keydown", tecla);
    return () => window.removeEventListener("keydown", tecla);
  }, []);

  // La luz de la fila se apaga en cuanto se toca otra cosa: ya ha dicho dónde está. Se
  // engancha un instante después para no cazar el mismo Enter o clic que la encendió.
  useEffect(() => {
    if (!encendida) return;
    const apagar = () => setEncendida(null);
    const espera = window.setTimeout(() => {
      window.addEventListener("pointerdown", apagar);
      window.addEventListener("keydown", apagar);
    }, 0);
    return () => {
      window.clearTimeout(espera);
      window.removeEventListener("pointerdown", apagar);
      window.removeEventListener("keydown", apagar);
    };
  }, [encendida]);

  const refrescar = useCallback(() => {
    void getSettings().then(setLocal);
    void shortcutStatus().then(setShortcuts);
    void cacheStats().then(setCache);
    void printScreenState().then(setImprPant);
    void capturaModerna()
      .then(setModernaOk)
      .catch(() => {
        // Una compilacion sin este comando: no se avisa de nada, que es lo de antes.
      });
    void replayStatus()
      .then((estado) => estado && setReplay(estado))
      .catch(() => {
        // Una compilacion sin este comando: la fila sale como si estuviera apagado.
      });
    // Las pantallas se preguntan al abrir los ajustes y no al arrancar la app: enchufar o
    // quitar un monitor es justo lo que pasa entre una vez y otra.
    void listScreens()
      .then((lista) => lista && setPantallas(lista))
      .catch(() => setPantallas([]));
    // S2 (grabación): igual que las pantallas, una webcam se enchufa entre una vez y otra.
    void listCameras()
      .then((lista) => setCamaras(lista ?? []))
      .catch(() => setCamaras([]));
  }, []);

  // Cerrar los ajustes solo esconde la ventana, nunca la destruye, asi que esto se
  // monta una vez por sesion. Sin volver a preguntar al reaparecer, el tamanno de la
  // cache se quedaba en el que tenia al arrancar y el boton de vaciar, muerto.
  useEffect(() => {
    refrescar();
    setError(null);
    // Se pregunta una sola vez y en un solo sitio: en Rust la respuesta se consume al
    // leerla, asi que dos preguntas dejarian a una de las dos sin enterarse.
    void justUpdated()
      .then((si) => {
        if (!si) return;
        setRecienActualizado(true);
        setSeccion("ayuda");
      })
      .catch(() => {
        // Una compilacion sin este comando: se abre donde siempre.
      });
    const unlisten = listen(EVENTS.settingsShown, () => {
      refrescar();
      setError(null);
    });
    // El anillo cambia por su cuenta: se enciende al arrancar la app, va llenandose y
    // guarda cuando alguien pulsa la tecla. Preguntando solo al abrir la ventana, la
    // fila se quedaria contando lo de hace un rato.
    const unReplay = listen<ReplayStatus>(EVENTS.replay, (evento) => {
      setReplay(evento.payload);
      // S45 (Linux): si se ha apagado sin que nadie tocara el interruptor (el escritorio dejó
      // de compartir la pantalla), Rust ya ha dejado el ajuste en apagado: se relee, o el
      // interruptor seguiría en «sí» sobre un anillo que no graba. En Windows no llega nunca.
      if (evento.payload.apagadoPor) {
        void getSettings().then(setLocal);
        void shortcutStatus().then(setShortcuts);
      }
    });
    return () => {
      void unlisten.then((fn) => fn());
      void unReplay.then((fn) => fn());
    };
  }, [refrescar]);

  // Lo que el aviso no cuenta: Rust solo avisa al encender, al apagar y al rescatar, y al
  // encenderlo el anillo lleva cero segundos y aún no ha escrito nada. Sin preguntar, la fila
  // se quedaba en «0 B/s» y «ahora mismo hay 0 s guardados» justo cuando alguien la mira para
  // ver si funciona. Se pregunta cada segundo, y solo con la ventana delante: escondida o
  // detrás no la ve nadie, y el anillo corre toda la tarde.
  useEffect(() => {
    if (!replay.running) return;
    const cada = window.setInterval(() => {
      if (document.visibilityState === "hidden" || !document.hasFocus()) return;
      void replayStatus()
        .then((estado) => estado && setReplay(estado))
        .catch(() => {});
    }, 1_000);
    return () => window.clearInterval(cada);
  }, [replay.running]);

  // El ultimo valor conocido, para poder guardar fuera del updater de useState.
  const ultimo = useRef<Settings | null>(null);
  ultimo.current = settings;

  /**
   * A qué tamaño va a grabar el anillo con lo que hay elegido.
   *
   * Se calcula aquí y no se espera a que Rust lo diga porque hay que enseñarlo **antes** de
   * encenderlo: es la mitad de la respuesta a «cuánto me va a costar esto».
   */
  const medidaGrabada = useMemo(() => {
    const pantalla =
      pantallas.find((p) => p.id === settings?.replayScreen) ??
      pantallas.find((p) => p.isPrimary) ??
      pantallas[0];
    const nativo = { ancho: pantalla?.width ?? 1920, alto: pantalla?.height ?? 1080 };
    const pedido = settings?.replayHeight ?? 0;
    if (pedido === 0 || pedido >= nativo.alto) return nativo;
    return {
      ancho: Math.round((nativo.ancho * pedido) / nativo.alto / 2) * 2,
      alto: Math.floor(pedido / 2) * 2,
    };
  }, [pantallas, settings?.replayScreen, settings?.replayHeight]);

  const patch = useCallback((partial: Partial<Settings>) => {
    const prev = ultimo.current;
    if (!prev) return;
    const next = { ...prev, ...partial };
    // Guardar dentro del updater parecia mas corto, pero React lo llama dos veces y
    // salian dos escrituras y dos re-registros de atajos por cada cambio.
    ultimo.current = next;
    setError(null);
    setLocal(next);
    void setSettings(next)
      .then(() => shortcutStatus().then(setShortcuts))
      .catch((e) => {
        setError(String(e));
        // Rust puede haber rechazado parte del cambio (encender el anillo y no poder).
        // Sin releer, el interruptor se quedaria en «si» sobre algo que no esta pasando.
        void getSettings().then((frescos) => {
          ultimo.current = frescos;
          setLocal(frescos);
        });
        void replayStatus()
          .then((estado) => estado && setReplay(estado))
          .catch(() => {});
      });
  }, []);

  const cambiarImprPant = useCallback(async (quiere: boolean) => {
    setError(null);
    try {
      setImprPant(await usePrintScreen(quiere));
      // usePrintScreen escribe los ajustes en Rust. Sin volver a leerlos, el proximo
      // cambio de cualquier otra fila reenviaria el valor viejo y desharia esto.
      const frescos = await getSettings();
      ultimo.current = frescos;
      setLocal(frescos);
      setShortcuts(await shortcutStatus());
    } catch (e) {
      setError(String(e));
    }
  }, []);

  // Desinstalar algo no puede pasar de un clic despistado: el primero pregunta y el
  // segundo lo hace. Y a los pocos segundos se olvida, para no dejar la trampa armada.
  const quitarRecortes = useCallback(async () => {
    if (recortes !== "confirmar") {
      setRecortes("confirmar");
      window.setTimeout(() => setRecortes((r) => (r === "confirmar" ? null : r)), 5000);
      return;
    }
    setRecortes("quitando");
    try {
      const habia = await removeSnippingTool();
      setRecortes(habia ? "quitada" : "ya no estaba");
    } catch (e) {
      setRecortes(null);
      setError(String(e));
    }
  }, [recortes]);

  // Va por su cuenta porque es lo unico que le quita algo al usuario: la lista de atajos
  // de Windows va por letra, asi que apagar Win+Mayus+S apaga tambien Win+S.
  const cambiarWinShiftS = useCallback(async (quiere: boolean) => {
    setError(null);
    try {
      await useWinShiftS(quiere);
      const frescos = await getSettings();
      ultimo.current = frescos;
      setLocal(frescos);
      setShortcuts(await shortcutStatus());
    } catch (e) {
      setError(String(e));
    }
  }, []);

  // Reiniciar el Explorador es lo que hace que Windows relea la lista de teclas apagadas.
  // Antes esto era "cierra sesión y vuelve a entrar", que nadie hace por un atajo, y hasta
  // entonces la tecla no era de nadie: ni la abría Windows ni la cogía winshotx.
  const aplicarTecla = useCallback(async () => {
    setError(null);
    setAplicando(true);
    try {
      setShortcuts(await restartShell());
    } catch (e) {
      setError(String(e));
    } finally {
      setAplicando(false);
    }
  }, []);

  const vaciarCache = useCallback(() => {
    void clearCache()
      .then(setCache)
      .catch((e) => setError(String(e)));
  }, []);

  // S40: la ventana mide siempre lo mismo (`ventanaFija.ts`, desde `App.tsx`) y lo que no
  // cabe en ella (el historial lleno, una pantalla baja) baja con la rueda en este hueco, con
  // la cabecera y el riel quietos.
  const hueco = useRef<HTMLDivElement>(null);
  const contenido = useRef<HTMLDivElement>(null);
  // S21: hasta saber dónde corre tampoco se pinta nada.
  const sobra = useSobra(hueco, contenido, settings !== null && donde !== null);

  // Al cambiar de apartado se empieza por arriba: el hueco con rueda es el mismo para todos,
  // y sin esto el historial abría a media galería si se había bajado en otro apartado.
  const ir = useCallback((id: ApartadoId) => {
    setSeccion(id);
    setEncendida(null);
    hueco.current?.scrollTo?.({ top: 0 });
  }, []);

  // Elegir pantalla solo sale con más de una (y en Wayland nunca: la elige la persona en la
  // pregunta del escritorio): el buscador no ofrece una fila que no está. Y en Linux tampoco
  // lo que solo existe en Windows.
  const hay = useCallback(
    (id: string) =>
      (id !== "ultPantalla" || (pantallas.length > 1 && !enWayland)) && !(enLinux && SOLO_EN_WINDOWS.has(id)),
    [pantallas, enLinux, enWayland],
  );

  // Hasta saber dónde corre no se pinta nada: en Linux se verían un instante las teclas de Windows.
  if (!settings || !donde) {
    return (
      <div className="flex h-full items-center justify-center bg-lienzo text-sm text-tenue">
        Cargando…
      </div>
    );
  }

  const datos: DatosAjustes = {
    enLinux,
    enWayland,
    teclasDelEscritorio,
    teclasAMano,
    ordenDeLaApp,
    cambiarTeclas,
    cambiarTeclaDeRescate,
    sonido,
    obturadorDelEscritorio: donde.obturadorDelEscritorio ?? false,
    settings,
    patch,
    shortcuts,
    replay,
    pantallas,
    cache,
    vaciarCache,
    imprPant,
    cambiarImprPant: (v) => void cambiarImprPant(v),
    recortes,
    quitarRecortes: () => void quitarRecortes(),
    aplicando,
    aplicarTecla: () => void aplicarTecla(),
    cambiarWinShiftS: (v) => void cambiarWinShiftS(v),
    modernaOk,
    camaras,
    medidaGrabada,
    onVerBienvenida,
    onTour: () => setTour(true),
    version: VERSION,
    actualizacion: {
      fase: actualizacion.fase,
      texto: actualizacion.texto,
      instalar: actualizacion.instalar,
      mirar: () => void actualizacion.mirar(true),
    },
  };

  const actual = datosDe(seccion);
  const Filas = APARTADO[seccion];

  return (
    <div className="relative flex h-full flex-col overflow-hidden bg-lienzo">
      <Cabecera
        version={VERSION}
        onBuscar={() => setPaleta(true)}
        onSalir={() => void quitApp()}
        aviso={
          <AvisoActualizacion
            fase={actualizacion.fase}
            texto={actualizacion.texto}
            instalar={actualizacion.instalar}
            mirar={() => void actualizacion.mirar(true)}
          />
        }
      />

      <div className="flex min-h-0 flex-1">
        <Riel activo={seccion} onIr={ir} sinTeclasDeWindows={enLinux} />

        <div className="relative min-w-0 flex-1">
          <div ref={hueco} className="h-full overflow-y-auto" data-scroll-ajustes>
            {/* Al menos el alto del hueco: así, en un apartado corto, el dibujo de la derecha
                llega hasta abajo y no queda una franja vacía debajo de todo (S40). */}
            <div ref={contenido} data-contenido-ajustes data-apartado={seccion} className="flex min-h-full flex-col">
              {seccion === "historial" ? (
                <div className="flex-1 px-5 py-5">
                  <Historial carpeta={settings.saveDirectory} />
                </div>
              ) : (
                <div className="flex flex-1 gap-7 px-6 pb-7 pt-6">
                  <main className="min-w-0 flex-1">
                    <div className="mb-4">
                      <h1 className="text-[22px] font-bold leading-tight text-titulo">{t(actual.nombre)}</h1>
                      <p className="mt-1 text-[14px] text-apagado">{t(actual.frase)}</p>
                    </div>
                    <FilaEncendida.Provider value={encendida}>
                      <Filas {...datos} />
                    </FilaEncendida.Provider>
                  </main>
                  {actual.dibujo && (
                    // De arriba abajo, también en un apartado corto: la ventana mide siempre
                    // lo mismo (S40) y así no queda una franja vacía debajo de todo. Lo de
                    // dentro va centrado (con el nombre arriba y el dibujo en medio quedaba un
                    // hueco entre los dos, mirado en foto).
                    <aside
                      data-tour="dibujo"
                      data-dibujo-de={seccion}
                      className="flex w-[290px] shrink-0 flex-col justify-center rounded-xl bg-tarjeta px-5 py-6"
                    >
                      <p className="mb-4 text-[15px] font-bold text-titulo">{t("Así va a salir")}</p>
                      <VistaPrevia
                        apartado={seccion}
                        settings={settings}
                        replay={replay}
                        enLinux={enLinux}
                        enWayland={enWayland}
                        sonido={sonido}
                        teclasDelEscritorio={teclasDelEscritorio}
                        teclasAMano={teclasAMano}
                        ordenDeLaApp={ordenDeLaApp}
                      />
                    </aside>
                  )}
                </div>
              )}
            </div>
          </div>
          {/* El desvanecido dice «esto sigue»: cortado a hueso, lo de abajo no se busca. Solo
              cuando de verdad sigue: cabiendo, tapaba el borde de abajo de las filas. */}
          {sobra && (
            <div className="pointer-events-none absolute inset-x-0 bottom-0 h-6 bg-gradient-to-t from-lienzo to-transparent" />
          )}
        </div>
      </div>

      {paleta && (
        <Paleta
          version={VERSION}
          hay={hay}
          enLinux={enLinux}
          onCerrar={() => setPaleta(false)}
          onElegir={(b) => {
            setPaleta(false);
            ir(b.apartado);
            // El historial es un apartado entero, sin una fila que iluminar.
            if (b.apartado !== "historial") setEncendida(b.id);
          }}
        />
      )}

      {tour && (
        <GuidedTour onNavegar={ir} onCerrar={() => setTour(false)} sinTeclasDeWindows={enLinux} />
      )}

      {/* Fuera del hueco: un fallo escondido al final de una columna no se ve. */}
      {error && (
        <p className="mx-4 mb-3 shrink-0 rounded-lg border border-red-500/25 bg-red-500/10 px-3 py-2 text-[13px] text-[color:light-dark(#b91c1c,#f87171)]">
          {error}
        </p>
      )}

      {/* Lo que se toca a menudo, a mano desde cualquier apartado (vuelve en S40). */}
      <BarraRapida
        settings={settings}
        patch={patch}
        onTour={() => setTour(true)}
        actualizacion={{ fase: actualizacion.fase, mirar: () => void actualizacion.mirar(true) }}
      />
    </div>
  );
}
