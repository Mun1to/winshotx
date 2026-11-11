import { useCallback, useEffect, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { Copy, FolderOpen, Image as ImagenIcono, Trash2, Video } from "lucide-react";
import {
  historialAbrir,
  historialBorrar,
  historialCopiar,
  historialLista,
  historialMiniatura,
  historialMostrar,
  openFolder,
  type EntradaHistorial,
} from "../../lib/ipc";
import { formatBytes } from "../../lib/format";
import { idiomaActivo, useT } from "../../lib/i18n";

/**
 * El historial de fotos y vídeos, dentro de la app (E6 del plan de CleanShot X).
 *
 * Es la carpeta de capturas puesta en una rejilla: lo nuevo arriba, con su miniatura (la
 * misma que da el Explorador) y su fecha. Un clic la abre con el programa de siempre, y al
 * pasar el ratón salen copiar, enseñarla en su carpeta y mandarla a la papelera.
 *
 * Vive en una pestaña de los ajustes porque es la ventana de winshotx que ya existe: una
 * ventana más solo para esto sería otra cosa que abrir, colocar y cerrar.
 *
 * **Pide de 48 en 48.** Una carpeta de capturas de un año son miles de archivos, y pintarlos
 * todos de golpe (cada uno con su miniatura) congelaría la ventana para enseñar lo que
 * nadie va a mirar. Lo de abajo llega con «Ver más».
 */
const POR_PAGINA = 48;

type Filtro = "todo" | "foto" | "video";

const FILTROS: { id: Filtro; nombre: string }[] = [
  { id: "todo", nombre: "Todo" },
  { id: "foto", nombre: "Fotos" },
  { id: "video", nombre: "Vídeos" },
];

export function Historial({ carpeta }: { carpeta: string }) {
  const t = useT();
  const [filtro, setFiltro] = useState<Filtro>("todo");
  const [entradas, setEntradas] = useState<EntradaHistorial[] | null>(null);
  const [total, setTotal] = useState(0);
  const [aviso, setAviso] = useState<{ texto: string; malo: boolean } | null>(null);

  const cargar = useCallback(
    async (desde: number) => {
      try {
        const pagina = await historialLista(filtro === "todo" ? null : filtro, desde, POR_PAGINA);
        // Con la lista vacía si Rust contesta otra cosa: el historial no puede tumbar los ajustes.
        const nuevas = Array.isArray(pagina?.entradas) ? pagina.entradas : [];
        setEntradas((antes) => (desde === 0 ? nuevas : [...(antes ?? []), ...nuevas]));
        setTotal(pagina?.total ?? nuevas.length);
      } catch (e) {
        setEntradas((antes) => antes ?? []);
        setAviso({ texto: String(e), malo: true });
      }
    },
    [filtro],
  );

  useEffect(() => {
    setEntradas(null);
    void cargar(0);
  }, [cargar]);

  useEffect(() => {
    if (!aviso) return;
    const reloj = window.setTimeout(() => setAviso(null), aviso.malo ? 3500 : 1600);
    return () => window.clearTimeout(reloj);
  }, [aviso]);

  // Sin aviso si no hay nada que contar: enseñar en la carpeta ya se ve, y un aviso vacío
  // borraría el de lo anterior (un «Copiado» que no llegaba a leerse).
  const hacer = async (accion: () => Promise<unknown>, bien: string | null) => {
    try {
      await accion();
      if (bien) setAviso({ texto: bien, malo: false });
      return true;
    } catch (e) {
      setAviso({ texto: String(e), malo: true });
      return false;
    }
  };

  const borrar = async (entrada: EntradaHistorial) => {
    if (await hacer(() => historialBorrar(entrada.ruta), "Enviada a la papelera")) {
      setEntradas((antes) => (antes ?? []).filter((e) => e.ruta !== entrada.ruta));
      setTotal((n) => Math.max(0, n - 1));
    }
  };

  return (
    <section className="relative flex flex-col gap-4" data-historial>
      <div className="flex items-end justify-between gap-4 border-b border-linea">
        <div role="tablist" aria-label={t("Qué enseñar")} className="flex gap-5">
          {FILTROS.map((f) => (
            <button
              key={f.id}
              type="button"
              role="tab"
              aria-selected={filtro === f.id}
              onClick={() => setFiltro(f.id)}
              className={`-mb-px border-b-[3px] pb-2 text-[15px] font-semibold transition-colors ${
                filtro === f.id
                  ? "border-marca text-titulo"
                  : "border-transparent text-suave hover:text-titulo"
              }`}
            >
              {t(f.nombre)}
            </button>
          ))}
        </div>
        <button
          type="button"
          onClick={() => void openFolder(carpeta)}
          className="mb-2 flex h-8 items-center gap-1.5 rounded-md bg-realce px-3 text-[13px] font-medium text-titulo transition-colors hover:bg-activo"
        >
          <FolderOpen className="size-4" />
          {t("Abrir la carpeta")}
        </button>
      </div>

      {entradas === null ? (
        <Esqueleto />
      ) : entradas.length === 0 ? (
        <Vacio filtro={filtro} />
      ) : (
        <>
          <ul className="grid grid-cols-[repeat(auto-fill,minmax(170px,1fr))] gap-x-3 gap-y-4">
            {entradas.map((e) => (
              <Carta
                key={e.ruta}
                entrada={e}
                onCopiar={() => void hacer(() => historialCopiar(e.ruta), e.tipo === "foto" ? "Copiada" : "Copiado")}
                onMostrar={() => void hacer(() => historialMostrar(e.ruta), null)}
                onBorrar={() => void borrar(e)}
                onAbrir={() => void historialAbrir(e.ruta).catch((err) => setAviso({ texto: String(err), malo: true }))}
              />
            ))}
          </ul>
          {entradas.length < total && (
            <button
              type="button"
              onClick={() => void cargar(entradas.length)}
              className="mx-auto h-9 rounded-md bg-realce px-4 text-[13.5px] font-medium text-titulo transition-colors hover:bg-activo"
            >
              {t("Ver más")}
            </button>
          )}
        </>
      )}

      {aviso && (
        <span
          role="status"
          className={`fixed bottom-4 left-1/2 -translate-x-1/2 rounded-md px-3 py-1.5 text-[13px] font-medium text-white shadow-lg ${
            aviso.malo ? "bg-red-500/95" : "bg-emerald-600/95"
          }`}
        >
          {t(aviso.texto)}
        </span>
      )}
    </section>
  );
}

/**
 * Las miniaturas se piden de cuatro en cuatro. La primera vez Windows tiene que sacar
 * cada una del archivo, y pedir cuarenta y ocho a la vez se comería la máquina un momento
 * para pintar algo que va llegando igual.
 */
const cola: (() => void)[] = [];
let enMarcha = 0;
function enCola<T>(trabajo: () => Promise<T>): Promise<T> {
  return new Promise((listo, falla) => {
    const arrancar = () => {
      enMarcha++;
      trabajo()
        .then(listo, falla)
        .finally(() => {
          enMarcha--;
          cola.shift()?.();
        });
    };
    if (enMarcha < 4) arrancar();
    else cola.push(arrancar);
  });
}

function Carta({
  entrada,
  onAbrir,
  onCopiar,
  onMostrar,
  onBorrar,
}: {
  entrada: EntradaHistorial;
  onAbrir: () => void;
  onCopiar: () => void;
  onMostrar: () => void;
  onBorrar: () => void;
}) {
  const t = useT();
  const [miniatura, setMiniatura] = useState<string | null>(null);
  const [sinMiniatura, setSinMiniatura] = useState(false);
  // Borrar pide dos clics: el primero lo pregunta y el segundo lo hace. A los pocos
  // segundos se olvida, para no dejar la trampa armada.
  const [confirmar, setConfirmar] = useState(false);

  useEffect(() => {
    let vivo = true;
    void enCola(() => historialMiniatura(entrada.ruta))
      .then((ruta) => vivo && ruta && setMiniatura(convertFileSrc(ruta)))
      .catch(() => vivo && setSinMiniatura(true));
    return () => {
      vivo = false;
    };
  }, [entrada.ruta]);

  useEffect(() => {
    if (!confirmar) return;
    const reloj = window.setTimeout(() => setConfirmar(false), 4000);
    return () => window.clearTimeout(reloj);
  }, [confirmar]);

  const TipoIcono = entrada.tipo === "foto" ? ImagenIcono : Video;

  return (
    <li className="group relative" data-entrada={entrada.nombre}>
      <button
        type="button"
        onClick={onAbrir}
        title={entrada.nombre}
        aria-label={t("Abrir {nombre}", { nombre: entrada.nombre })}
        className="block aspect-video w-full overflow-hidden rounded-md border-2 border-linea bg-hueco transition-colors hover:border-marca focus:border-marca focus:outline-none"
      >
        {miniatura ? (
          <img src={miniatura} alt="" draggable={false} className="size-full object-contain" />
        ) : (
          <span className="flex size-full items-center justify-center text-tenue">
            {sinMiniatura && <TipoIcono className="size-6" />}
          </span>
        )}
      </button>

      {/* Lo secundario, al pasar el ratón; abrir es la carta entera. */}
      <span
        className={`absolute right-1.5 top-1.5 flex gap-1 transition-opacity duration-150 ${
          confirmar ? "opacity-100" : "opacity-0 group-hover:opacity-100 group-focus-within:opacity-100"
        }`}
      >
        <Accion icono={Copy} etiqueta={t("Copiar")} onClick={onCopiar} />
        <Accion icono={FolderOpen} etiqueta={t("Mostrar en la carpeta")} onClick={onMostrar} />
        {confirmar ? (
          <button
            type="button"
            onClick={() => {
              setConfirmar(false);
              onBorrar();
            }}
            className="flex h-7 items-center rounded-md bg-red-500 px-2 text-[12.5px] font-semibold text-white"
          >
            {t("¿Borrar?")}
          </button>
        ) : (
          <Accion icono={Trash2} etiqueta={t("Mandar a la papelera")} onClick={() => setConfirmar(true)} peligro />
        )}
      </span>

      <p className="mt-1.5 flex items-center gap-1.5 text-[13px] text-texto">
        <TipoIcono className="size-3.5 shrink-0 text-suave" aria-label={t(entrada.tipo === "foto" ? "Foto" : "Vídeo")} />
        <span className="truncate">{cuando(entrada.cuandoMs, t)}</span>
        <span className="ml-auto shrink-0 tabular-nums text-suave">{formatBytes(entrada.bytes)}</span>
      </p>
    </li>
  );
}

function Accion({
  icono: Dibujo,
  etiqueta,
  onClick,
  peligro = false,
}: {
  icono: typeof Copy;
  etiqueta: string;
  onClick: () => void;
  peligro?: boolean;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      title={etiqueta}
      aria-label={etiqueta}
      className={`flex size-7 items-center justify-center rounded-md bg-neutral-900/85 text-neutral-100 transition-colors ${
        peligro ? "hover:bg-red-500" : "hover:bg-neutral-700"
      }`}
    >
      <Dibujo className="size-4" />
    </button>
  );
}

/**
 * Cuándo se guardó, como lo diría una persona: «Hoy, 10:42», «Ayer, 18:05» o «3 nov,
 * 09:15». Con el año solo si no es este.
 */
export function cuando(ms: number, t: (es: string) => string, ahora = new Date()): string {
  const fecha = new Date(ms);
  const idioma = { es: "es-ES", en: "en-GB", zh: "zh-CN" }[idiomaActivo()];
  const hora = fecha.toLocaleTimeString(idioma, { hour: "2-digit", minute: "2-digit" });
  const dia = (d: Date) => new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
  const dias = Math.round((dia(ahora) - dia(fecha)) / 86_400_000);
  if (dias === 0) return `${t("Hoy")}, ${hora}`;
  if (dias === 1) return `${t("Ayer")}, ${hora}`;
  const conAnno = fecha.getFullYear() !== ahora.getFullYear();
  const fechaCorta = fecha.toLocaleDateString(idioma, {
    day: "numeric",
    month: "short",
    ...(conAnno ? { year: "numeric" } : {}),
  });
  return `${fechaCorta}, ${hora}`;
}

/** La forma de la rejilla mientras llega la lista, sin ruedita en medio. */
function Esqueleto() {
  return (
    <ul className="grid grid-cols-[repeat(auto-fill,minmax(170px,1fr))] gap-x-3 gap-y-4" aria-hidden="true">
      {Array.from({ length: 8 }, (_, i) => (
        <li key={i}>
          <span className="block aspect-video w-full animate-pulse rounded-md bg-realce" />
          <span className="mt-2 block h-3 w-2/3 animate-pulse rounded bg-realce" />
        </li>
      ))}
    </ul>
  );
}

/** Sin nada que enseñar: suelto y en el centro del hueco, sin caja. */
function Vacio({ filtro }: { filtro: Filtro }) {
  const t = useT();
  const titulo =
    filtro === "foto"
      ? t("Todavía no hay fotos guardadas")
      : filtro === "video"
        ? t("Todavía no hay vídeos guardados")
        : t("Todavía no has guardado nada");
  return (
    <div className="flex flex-col items-center justify-center gap-1.5 py-20 text-center">
      <p className="text-[17px] font-semibold text-titulo">{titulo}</p>
      <p className="max-w-[46ch] text-[14px] text-suave">
        {t("Las fotos que guardes y los vídeos que exportes salen aquí, de lo más nuevo a lo más viejo.")}
      </p>
    </div>
  );
}
