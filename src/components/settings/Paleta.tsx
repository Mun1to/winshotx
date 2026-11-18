import { useEffect, useId, useMemo, useRef, useState } from "react";
import { Search } from "lucide-react";
import { useT } from "../../lib/i18n";
import { EN } from "../../lib/textos-en";
import { ZH } from "../../lib/textos-zh";
import { apartado, buscarAjustes, type AjusteBuscable } from "./mapaDeAjustes";

interface Props {
  version: string;
  /** Las filas que hoy no salen (elegir pantalla con una sola) no se ofrecen. */
  hay: (id: string) => boolean;
  /** S21 (Linux): las filas que allí se llaman de otra forma salen con ese nombre. */
  enLinux?: boolean;
  /** Ir al apartado del ajuste e iluminar su fila. */
  onElegir: (ajuste: AjusteBuscable) => void;
  onCerrar: () => void;
}

/**
 * La paleta del buscador (S36; es la de la variación 8, puesta en la cabecera de la 9).
 *
 * Se abre encima, donde está el campo de la cabecera, con su propio campo y la lista de lo
 * que encuentra: el icono del apartado, el nombre de la fila y el apartado a la derecha. Con
 * las flechas se elige, con Enter se va y con Esc se cierra. Ir es saltar al apartado e
 * iluminar la fila, no filtrar la página: así el ajuste se ve en su sitio, con sus vecinos.
 *
 * Busca en los tres idiomas a la vez (el nombre de cada fila y sus palabras), porque quien
 * tiene la app en inglés también puede escribir en español.
 */
export function Paleta({ version, hay, enLinux = false, onElegir, onCerrar }: Props) {
  const t = useT();
  const id = useId();
  const campo = useRef<HTMLInputElement>(null);
  const [consulta, setConsulta] = useState("");
  const [elegido, setElegido] = useState(0);
  const nombreDe = (b: AjusteBuscable) => (enLinux && b.nombreEnLinux) || b.nombre;

  const lista = useMemo(
    () =>
      buscarAjustes(
        consulta,
        // El nombre de la pantalla primero, y luego los de los otros dos idiomas.
        (b) => {
          const nombre = (enLinux && b.nombreEnLinux) || b.nombre;
          return [t(nombre, { version }), nombre, EN[nombre] ?? "", ZH[nombre] ?? ""];
        },
        hay,
      ),
    [consulta, t, version, hay, enLinux],
  );

  useEffect(() => campo.current?.focus(), []);

  // La elegida se mantiene a la vista al bajar con las flechas por una lista larga.
  useEffect(() => {
    document.getElementById(`${id}-${elegido}`)?.scrollIntoView?.({ block: "nearest" });
  }, [elegido, id]);

  const ir = (b: AjusteBuscable | undefined) => {
    if (b) onElegir(b);
  };

  return (
    <div
      className="absolute inset-0 z-30 flex items-start justify-center bg-black/40 px-6 pt-2"
      // Pulsar fuera la cierra, como cualquier ventanita que se abre encima.
      onPointerDown={(e) => {
        if (e.target === e.currentTarget) onCerrar();
      }}
    >
      <div
        role="dialog"
        aria-label={t("Buscar un ajuste")}
        data-paleta
        className="w-full max-w-[540px] overflow-hidden rounded-xl border border-linea-fuerte bg-flotante shadow-2xl"
      >
        <label className="flex items-center gap-3 border-b border-linea px-4 py-3.5">
          <Search className="size-[18px] shrink-0 text-apagado" strokeWidth={1.9} />
          <input
            ref={campo}
            role="combobox"
            aria-expanded={lista.length > 0}
            aria-controls={`${id}-lista`}
            aria-activedescendant={lista.length > 0 ? `${id}-${elegido}` : undefined}
            aria-label={t("Buscar un ajuste")}
            value={consulta}
            onChange={(e) => {
              setConsulta(e.target.value);
              setElegido(0);
            }}
            onKeyDown={(e) => {
              if (e.key === "Escape") {
                e.preventDefault();
                onCerrar();
              } else if (e.key === "ArrowDown") {
                e.preventDefault();
                setElegido((n) => Math.min(n + 1, Math.max(lista.length - 1, 0)));
              } else if (e.key === "ArrowUp") {
                e.preventDefault();
                setElegido((n) => Math.max(n - 1, 0));
              } else if (e.key === "Enter") {
                e.preventDefault();
                ir(lista[elegido]);
              }
            }}
            placeholder={t("Buscar un ajuste")}
            className="min-w-0 flex-1 bg-transparent text-[16px] text-titulo outline-none placeholder:text-tenue"
          />
          <span className="shrink-0 text-[12px] text-tenue">Esc</span>
        </label>
        <div id={`${id}-lista`} role="listbox" aria-label={t("Buscar un ajuste")} className="max-h-[300px] overflow-y-auto py-1.5">
          {consulta.trim() === "" ? (
            <p className="px-4 py-3 text-[14px] text-apagado">{t("Escribe lo que buscas: cursor, sonido, carpeta…")}</p>
          ) : lista.length === 0 ? (
            <p className="px-4 py-3 text-[14px] text-apagado">
              {t("Nada con «{consulta}». Prueba con otra palabra.", { consulta: consulta.trim() })}
            </p>
          ) : (
            lista.map((b, i) => {
              const a = apartado(b.apartado);
              return (
                <div
                  key={b.id}
                  id={`${id}-${i}`}
                  role="option"
                  aria-selected={i === elegido}
                  onPointerEnter={() => setElegido(i)}
                  // En el `pointerdown` y no en el clic: así el campo no pierde el foco antes.
                  onPointerDown={(e) => {
                    e.preventDefault();
                    ir(b);
                  }}
                  className={`flex cursor-pointer items-center gap-3 px-4 py-2.5 ${i === elegido ? "bg-realce" : ""}`}
                >
                  <a.icono className="size-[18px] shrink-0 text-apagado" strokeWidth={1.9} />
                  <span className="min-w-0 flex-1 text-[15px] font-medium text-titulo">{t(nombreDe(b), { version })}</span>
                  <span className="shrink-0 text-[13px] text-apagado">{t(a.nombre)}</span>
                </div>
              );
            })
          )}
        </div>
      </div>
    </div>
  );
}
