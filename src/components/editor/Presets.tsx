import { useEffect, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import { listen } from "@tauri-apps/api/event";
import { Check, ChevronDown, Pencil, Trash2 } from "lucide-react";
import { AVISO_PRESETS, getSettings, presetsGuardar, type PresetGuardado } from "../../lib/ipc";
import {
  borrar,
  esElMismo,
  guardarComo,
  indiceDe,
  leerPresets,
  paraGuardar,
  renombrar,
  type Estilo,
  type Preset,
} from "../../lib/presets";
import { useT } from "../../lib/i18n";

/** Los que caben, como en Rust (`presets::MAXIMO`). */
const MAXIMO = 50;

interface Props {
  /** El estilo que hay puesto ahora en el panel. Ver `estiloDe` en `presets.ts`. */
  estilo: Estilo;
  /** Poner un estilo guardado. Ver `panelDe` y `edicionDe` en `presets.ts`. */
  onAplicar: (estilo: Estilo) => void;
  /**
   * La lista siempre a la vista, una fila por estilo, y «Guardar estilo como…» debajo (S39).
   *
   * Es como va en su grupo del editor, «Estilos», que tiene sitio para ella: un estilo es
   * la suma del fondo, el puntero, los clics y el zoom, y guardarlo se lee DESPUÉS de haberlos
   * elegido. Sin esto, el botón con su desplegable de siempre.
   */
  enLista?: boolean;
}

/**
 * Los estilos guardados con nombre (D2): «Guardar estilo como…» y la lista para ponerlos.
 *
 * El botón de la lista dice el estilo que hay puesto, o «Sin guardar» si lo de ahora no es
 * ninguno; al abrirla, cada estilo se pone con un clic, y al pasar el ratón se le cambia el
 * nombre o se borra (borrar pide un segundo clic). La lista es propia y no el desplegable
 * del sistema, que no se puede vestir con la cara de la app.
 *
 * Vive sola: lee la lista de los ajustes, la guarda con `presetsGuardar` (que toca solo
 * esa lista) y escucha cuando cambia desde otra ventana.
 */
export function Presets({ estilo, onAplicar, enLista = false }: Props) {
  const t = useT();
  const [lista, setLista] = useState<Preset[]>([]);
  const [abierta, setAbierta] = useState(false);
  /** «Guardar como…» abierto, con el nombre que se va escribiendo. */
  const [nombre, setNombre] = useState<string | null>(null);
  /** El que se está renombrando, y el nombre nuevo. */
  const [renombrando, setRenombrando] = useState<{ indice: number; nombre: string } | null>(null);
  /** El que espera el segundo clic para borrarse. */
  const [porBorrar, setPorBorrar] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  const caja = useRef<HTMLDivElement>(null);

  useEffect(() => {
    let vivo = true;
    void getSettings()
      .then((ajustes) => {
        if (vivo) setLista(leerPresets(ajustes?.presets));
      })
      .catch(() => undefined);
    const quitar = listen<PresetGuardado[]>(AVISO_PRESETS, (e) => setLista(leerPresets(e.payload)));
    return () => {
      vivo = false;
      void quitar.then((fn) => fn());
    };
  }, []);

  // La lista se cierra al pulsar fuera, como cualquier menú.
  useEffect(() => {
    if (!abierta) return;
    const fuera = (e: PointerEvent) => {
      if (caja.current && !caja.current.contains(e.target as Node)) {
        setAbierta(false);
        setRenombrando(null);
        setPorBorrar(null);
      }
    };
    window.addEventListener("pointerdown", fuera);
    return () => window.removeEventListener("pointerdown", fuera);
  }, [abierta]);

  // El segundo clic de borrar tiene un rato: después se olvida, para no dejar la trampa armada.
  useEffect(() => {
    if (porBorrar === null) return;
    const reloj = window.setTimeout(() => setPorBorrar(null), 4000);
    return () => window.clearTimeout(reloj);
  }, [porBorrar]);

  async function guardar(nueva: Preset[]) {
    setError(null);
    try {
      const guardada = await presetsGuardar(paraGuardar(nueva));
      setLista(leerPresets(guardada ?? paraGuardar(nueva)));
      return true;
    } catch (e) {
      setError(String(e));
      return false;
    }
  }

  const puesto = lista.findIndex((p) => esElMismo(p.estilo, estilo));
  const escrito = nombre?.trim() ?? "";
  const reemplaza = escrito !== "" && indiceDe(lista, escrito) >= 0;
  const lleno = lista.length >= MAXIMO && !reemplaza;

  const guardarElNuevo = async () => {
    if (!escrito || lleno) return;
    if (await guardar(guardarComo(lista, escrito, estilo))) setNombre(null);
  };

  const teclaDeCampo = (enter: () => void, escape: () => void) => (e: KeyboardEvent<HTMLInputElement>) => {
    // Que Escape no le llegue al editor, que con Escape se cierra.
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      escape();
    } else if (e.key === "Enter") {
      e.preventDefault();
      enter();
    }
  };

  return (
    <div ref={caja} data-presets className="relative">
      {/* En su grupo: la lista primero y guardar lo de ahora debajo, que es su orden. */}
      {enLista ? (
        <>
      {(enLista ? lista.length > 0 : abierta) && (
            <ListaDeEstilos enLinea={enLista}>
              {lista.map((p, i) =>
                renombrando?.indice === i ? (
                  <li key={i} className="px-2 py-1">
                    <input
                      autoFocus
                      value={renombrando.nombre}
                      maxLength={60}
                      onChange={(e) => setRenombrando({ indice: i, nombre: e.target.value })}
                      onKeyDown={teclaDeCampo(
                        () => {
                          const otro = indiceDe(lista, renombrando.nombre);
                          if (!renombrando.nombre.trim() || (otro >= 0 && otro !== i)) {
                            setError(t("Ya hay otro estilo con ese nombre"));
                            return;
                          }
                          void guardar(renombrar(lista, i, renombrando.nombre)).then((bien) => bien && setRenombrando(null));
                        },
                        () => setRenombrando(null),
                      )}
                      onBlur={() => setRenombrando(null)}
                      aria-label={t("Nombre nuevo de «{nombre}»", { nombre: p.nombre })}
                      className="h-8 w-full rounded-md border border-marca bg-hueco px-2 text-[14px] text-titulo outline-none"
                    />
                  </li>
                ) : (
                  <li key={i} className="group relative flex items-center">
                    <button
                      type="button"
                      aria-current={puesto === i ? "true" : undefined}
                      onClick={() => {
                        if (porBorrar === i) return;
                        onAplicar(p.estilo);
                        setAbierta(false);
                      }}
                      className={`flex min-w-0 flex-1 items-center gap-2 py-2 pr-20 pl-3 text-left text-[14px] transition-colors hover:bg-realce ${
                        porBorrar === i ? "text-red-700 [[data-tema=oscuro]_&]:text-red-300" : "text-titulo"
                      }`}
                    >
                      <Check className={`size-4 shrink-0 ${puesto === i ? "text-marca" : "text-transparent"}`} aria-hidden="true" />
                      <span className="truncate">
                        {porBorrar === i ? t("Pulsa otra vez para borrar «{nombre}»", { nombre: p.nombre }) : p.nombre}
                      </span>
                    </button>
                    <span
                      className={`absolute right-1 flex items-center gap-0.5 transition-opacity ${
                        porBorrar === i ? "opacity-100" : "opacity-0 group-focus-within:opacity-100 group-hover:opacity-100"
                      }`}
                    >
                      <button
                        type="button"
                        aria-label={t("Cambiar el nombre de «{nombre}»", { nombre: p.nombre })}
                        title={t("Cambiar el nombre")}
                        onClick={() => {
                          setPorBorrar(null);
                          setRenombrando({ indice: i, nombre: p.nombre });
                        }}
                        className="grid size-8 place-items-center rounded-md text-suave transition-colors hover:bg-realce hover:text-titulo"
                      >
                        <Pencil className="size-4" />
                      </button>
                      <button
                        type="button"
                        aria-label={t("Borrar «{nombre}»", { nombre: p.nombre })}
                        title={t("Borrar")}
                        onClick={() => {
                          if (porBorrar === i) {
                            setPorBorrar(null);
                            void guardar(borrar(lista, i));
                          } else {
                            setPorBorrar(i);
                          }
                        }}
                        className={`grid size-8 place-items-center rounded-md transition-colors hover:bg-realce ${
                          porBorrar === i ? "text-red-700 [[data-tema=oscuro]_&]:text-red-300" : "text-suave hover:text-titulo"
                        }`}
                      >
                        <Trash2 className="size-4" />
                      </button>
                    </span>
                  </li>
                ),
              )}
            </ListaDeEstilos>
          )}
          {nombre === null && (
            <div className="flex">
                  <button
                    type="button"
                    onClick={() => {
                      setNombre("");
                      setAbierta(false);
                    }}
                    title={t("Guarda el fondo, el puntero, los clics y el zoom con un nombre, para ponérselos a otro vídeo")}
                    className={`flex h-9 items-center justify-center rounded-lg bg-realce px-3 text-[14px] font-medium whitespace-nowrap text-titulo transition-colors hover:bg-activo hover:text-titulo ${
                      lista.length === 0 || enLista ? "flex-1" : ""
                    }`}
                  >
                    {t("Guardar estilo como…")}
                  </button>
            </div>
          )}
      {nombre !== null && (
            <div className="mt-2 space-y-1.5">
              <div className="flex items-center gap-2">
                <input
                  autoFocus
                  value={nombre}
                  maxLength={60}
                  onChange={(e) => setNombre(e.target.value)}
                  onKeyDown={teclaDeCampo(() => void guardarElNuevo(), () => setNombre(null))}
                  placeholder={t("Nombre del estilo")}
                  aria-label={t("Nombre del estilo")}
                  className="h-9 min-w-0 flex-1 rounded-lg border border-linea-fuerte bg-hueco px-3 text-[14px] text-titulo outline-none placeholder:text-apagado focus:border-marca"
                />
                <button
                  type="button"
                  onClick={() => void guardarElNuevo()}
                  disabled={!escrito || lleno}
                  className="h-9 rounded-lg bg-[#0a76c9] px-3 text-[14px] font-semibold whitespace-nowrap text-white transition-colors hover:bg-[#0b84e0] disabled:opacity-40"
                >
                  {reemplaza ? t("Reemplazar") : t("Guardar")}
                </button>
                <button
                  type="button"
                  onClick={() => setNombre(null)}
                  className="h-9 rounded-lg px-2.5 text-[14px] font-medium whitespace-nowrap text-texto transition-colors hover:bg-realce hover:text-titulo"
                >
                  {t("Cancelar")}
                </button>
              </div>
              {reemplaza && <p className="text-[12px] text-suave">{t("Ya hay uno con ese nombre: se cambia por lo de ahora")}</p>}
              {lleno && <p className="text-[12px] text-aviso">{t("Caben {n} estilos: borra alguno para guardar otro", { n: MAXIMO })}</p>}
            </div>
          )}
        </>
      ) : (
        <>
          <div className="flex items-center gap-2">
        {lista.length > 0 && (
              <button
                type="button"
                aria-haspopup="true"
                aria-expanded={abierta}
                onClick={() => {
                  setAbierta((a) => !a);
                  setRenombrando(null);
                  setPorBorrar(null);
                }}
                className="flex h-9 min-w-0 flex-1 items-center justify-between gap-2 rounded-lg bg-realce px-3 text-[14px] font-medium text-titulo transition-colors hover:bg-activo hover:text-titulo"
              >
                <span className="truncate">{puesto >= 0 ? lista[puesto].nombre : t("Sin guardar")}</span>
                <ChevronDown
                  className={`size-4 shrink-0 text-suave transition-transform duration-200 ${abierta ? "rotate-180" : ""}`}
                />
              </button>
            )}
        {nombre === null && (
              <button
                type="button"
                onClick={() => {
                  setNombre("");
                  setAbierta(false);
                }}
                title={t("Guarda el fondo, el puntero, los clics y el zoom con un nombre, para ponérselos a otro vídeo")}
                className={`flex h-9 items-center justify-center rounded-lg bg-realce px-3 text-[14px] font-medium whitespace-nowrap text-titulo transition-colors hover:bg-activo hover:text-titulo ${
                  lista.length === 0 || enLista ? "flex-1" : ""
                }`}
              >
                {t("Guardar estilo como…")}
              </button>
            )}
          </div>
      {nombre !== null && (
            <div className="mt-2 space-y-1.5">
              <div className="flex items-center gap-2">
                <input
                  autoFocus
                  value={nombre}
                  maxLength={60}
                  onChange={(e) => setNombre(e.target.value)}
                  onKeyDown={teclaDeCampo(() => void guardarElNuevo(), () => setNombre(null))}
                  placeholder={t("Nombre del estilo")}
                  aria-label={t("Nombre del estilo")}
                  className="h-9 min-w-0 flex-1 rounded-lg border border-linea-fuerte bg-hueco px-3 text-[14px] text-titulo outline-none placeholder:text-apagado focus:border-marca"
                />
                <button
                  type="button"
                  onClick={() => void guardarElNuevo()}
                  disabled={!escrito || lleno}
                  className="h-9 rounded-lg bg-[#0a76c9] px-3 text-[14px] font-semibold whitespace-nowrap text-white transition-colors hover:bg-[#0b84e0] disabled:opacity-40"
                >
                  {reemplaza ? t("Reemplazar") : t("Guardar")}
                </button>
                <button
                  type="button"
                  onClick={() => setNombre(null)}
                  className="h-9 rounded-lg px-2.5 text-[14px] font-medium whitespace-nowrap text-texto transition-colors hover:bg-realce hover:text-titulo"
                >
                  {t("Cancelar")}
                </button>
              </div>
              {reemplaza && <p className="text-[12px] text-suave">{t("Ya hay uno con ese nombre: se cambia por lo de ahora")}</p>}
              {lleno && <p className="text-[12px] text-aviso">{t("Caben {n} estilos: borra alguno para guardar otro", { n: MAXIMO })}</p>}
            </div>
          )}
      {(enLista ? lista.length > 0 : abierta) && (
            <ListaDeEstilos enLinea={enLista}>
              {lista.map((p, i) =>
                renombrando?.indice === i ? (
                  <li key={i} className="px-2 py-1">
                    <input
                      autoFocus
                      value={renombrando.nombre}
                      maxLength={60}
                      onChange={(e) => setRenombrando({ indice: i, nombre: e.target.value })}
                      onKeyDown={teclaDeCampo(
                        () => {
                          const otro = indiceDe(lista, renombrando.nombre);
                          if (!renombrando.nombre.trim() || (otro >= 0 && otro !== i)) {
                            setError(t("Ya hay otro estilo con ese nombre"));
                            return;
                          }
                          void guardar(renombrar(lista, i, renombrando.nombre)).then((bien) => bien && setRenombrando(null));
                        },
                        () => setRenombrando(null),
                      )}
                      onBlur={() => setRenombrando(null)}
                      aria-label={t("Nombre nuevo de «{nombre}»", { nombre: p.nombre })}
                      className="h-8 w-full rounded-md border border-marca bg-hueco px-2 text-[14px] text-titulo outline-none"
                    />
                  </li>
                ) : (
                  <li key={i} className="group relative flex items-center">
                    <button
                      type="button"
                      aria-current={puesto === i ? "true" : undefined}
                      onClick={() => {
                        if (porBorrar === i) return;
                        onAplicar(p.estilo);
                        setAbierta(false);
                      }}
                      className={`flex min-w-0 flex-1 items-center gap-2 py-2 pr-20 pl-3 text-left text-[14px] transition-colors hover:bg-realce ${
                        porBorrar === i ? "text-red-700 [[data-tema=oscuro]_&]:text-red-300" : "text-titulo"
                      }`}
                    >
                      <Check className={`size-4 shrink-0 ${puesto === i ? "text-marca" : "text-transparent"}`} aria-hidden="true" />
                      <span className="truncate">
                        {porBorrar === i ? t("Pulsa otra vez para borrar «{nombre}»", { nombre: p.nombre }) : p.nombre}
                      </span>
                    </button>
                    <span
                      className={`absolute right-1 flex items-center gap-0.5 transition-opacity ${
                        porBorrar === i ? "opacity-100" : "opacity-0 group-focus-within:opacity-100 group-hover:opacity-100"
                      }`}
                    >
                      <button
                        type="button"
                        aria-label={t("Cambiar el nombre de «{nombre}»", { nombre: p.nombre })}
                        title={t("Cambiar el nombre")}
                        onClick={() => {
                          setPorBorrar(null);
                          setRenombrando({ indice: i, nombre: p.nombre });
                        }}
                        className="grid size-8 place-items-center rounded-md text-suave transition-colors hover:bg-realce hover:text-titulo"
                      >
                        <Pencil className="size-4" />
                      </button>
                      <button
                        type="button"
                        aria-label={t("Borrar «{nombre}»", { nombre: p.nombre })}
                        title={t("Borrar")}
                        onClick={() => {
                          if (porBorrar === i) {
                            setPorBorrar(null);
                            void guardar(borrar(lista, i));
                          } else {
                            setPorBorrar(i);
                          }
                        }}
                        className={`grid size-8 place-items-center rounded-md transition-colors hover:bg-realce ${
                          porBorrar === i ? "text-red-700 [[data-tema=oscuro]_&]:text-red-300" : "text-suave hover:text-titulo"
                        }`}
                      >
                        <Trash2 className="size-4" />
                      </button>
                    </span>
                  </li>
                ),
              )}
            </ListaDeEstilos>
          )}
        </>
      )}

      {error && <p className="mt-1.5 text-[12px] text-red-700 [[data-tema=oscuro]_&]:text-red-300">{error}</p>}
    </div>
  );
}

/** La lista que se abre debajo del botón, entrando poco a poco en vez de aparecer de golpe. */
function ListaDeEstilos({ children, enLinea = false }: { children: ReactNode; enLinea?: boolean }) {
  const t = useT();
  const [dentro, setDentro] = useState(false);
  useEffect(() => {
    const id = requestAnimationFrame(() => setDentro(true));
    return () => cancelAnimationFrame(id);
  }, []);
  return (
    <ul
      aria-label={t("Estilos guardados")}
      className={enLinea ? "mb-3 -mx-1 divide-y divide-linea-tenue" : `absolute top-10 right-0 left-0 z-30 max-h-[260px] origin-top overflow-y-auto rounded-lg border border-linea bg-flotante py-1 shadow-xl shadow-black/40 transition-[opacity,transform] duration-[180ms] ease-out ${
        dentro ? "scale-100 opacity-100" : "scale-[0.97] opacity-0"
      }`}
    >
      {children}
    </ul>
  );
}
