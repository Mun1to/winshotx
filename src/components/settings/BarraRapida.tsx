import { useState } from "react";
import { Coffee, Compass, RefreshCw } from "lucide-react";
import { Segmented } from "../ui/Segmented";
import { aplicarIdioma, idiomaActivo, useT } from "../../lib/i18n";
import { CAFE, comoSeLee } from "../../lib/enlaces";
import { openUrl } from "../../lib/ipc";
import { aplicarTema } from "../../lib/tema";
import type { CaptureFlow, Language, Settings, Theme } from "../../lib/types";
import type { Fase } from "./Actualizacion";

interface Props {
  settings: Settings;
  patch: (cambio: Partial<Settings>) => void;
  onTour: () => void;
  actualizacion: { fase: Fase; mirar: () => void };
}

/**
 * La barra de abajo: lo que se toca a menudo, a mano desde cualquier apartado.
 *
 * Estuvo de la 0.3 a la 0.4.7, S36 la quitó al rehacer los ajustes, y Munito la echó de menos
 * con la 0.5.0 delante: «no entiendo por qué no está el menú de abajo». Vuelve en S40 con el
 * estilo nuevo: opciones en texto con su raya (nada de pastillas), nada por debajo de 13 px,
 * sin verde y sin cajas.
 *
 * Se gana el sitio porque HACE cosas: el tema, el idioma, qué pasa al soltar el ratón, el tour,
 * buscar versión nueva y el café («Apoyar», que Munito también quería aquí). Los mismos ajustes
 * siguen en su apartado con su explicación entera; esto es el atajo, no el sitio donde viven.
 *
 * Lo de actualizar, repartido sin repetir nada: cuando HAY algo que decir (versión nueva,
 * bajando, falta reiniciar, un fallo) lo dice la cabecera, que es lo primero que se ve y lo que
 * eligió Munito en la variación 9; aquí está el botón de buscar y su respuesta.
 */
export function BarraRapida({ settings, patch, onTour, actualizacion }: Props) {
  const t = useT();

  const temas: { value: Theme; label: string }[] = [
    { value: "sistema", label: t("Automático") },
    { value: "claro", label: t("Claro") },
    { value: "oscuro", label: t("Oscuro") },
  ];

  // Cada idioma con su nombre en ESE idioma, que es como lo busca quien lo necesita (y ni
  // «ES / EN» ni un globo, que no dicen nada). «Automático» se queda en «La app»: aquí, dos
  // «Automático» seguidos no se distinguían. Va marcado el que se está viendo.
  const idiomas: { value: Exclude<Language, "sistema">; label: string }[] = [
    { value: "es", label: "Español" },
    { value: "en", label: "English" },
    { value: "zh", label: "中文" },
  ];

  // El ajuste que más se cambia de todos: la barra para elegir, o copiar sin preguntar.
  const soltar: { value: CaptureFlow; label: string }[] = [
    { value: "toolbar", label: t("Barra") },
    { value: "instant", label: t("Copia") },
  ];

  return (
    <footer
      data-barra-abajo
      className="flex h-11 shrink-0 items-center gap-6 border-t border-linea px-5 max-[880px]:gap-4 max-[880px]:px-3"
    >
      <Grupo nombre={t("Tema")}>
        <Segmented
          compacto
          etiqueta={t("Tema")}
          value={settings.theme}
          options={temas}
          onChange={(v) => {
            // Igual que en su fila: se pinta en el acto y se guarda después, que esperar a
            // Rust para cambiar de color hace que el botón parezca roto.
            aplicarTema(v);
            patch({ theme: v });
          }}
        />
      </Grupo>
      <Grupo nombre={t("Idioma")}>
        <Segmented
          compacto
          etiqueta={t("Idioma")}
          value={idiomaActivo()}
          options={idiomas}
          onChange={(v) => {
            aplicarIdioma(v);
            patch({ language: v });
          }}
        />
      </Grupo>
      <Grupo nombre={t("Al soltar")} siempre>
        <Segmented
          compacto
          etiqueta={t("Al soltar el ratón")}
          value={settings.captureFlow}
          options={soltar}
          onChange={(v) => patch({ captureFlow: v })}
        />
      </Grupo>
      <button
        type="button"
        onClick={onTour}
        title={t("Tour de los ajustes")}
        className="flex shrink-0 items-center gap-1.5 text-[13px] font-semibold whitespace-nowrap text-apagado transition-colors hover:text-titulo"
      >
        <Compass className="size-4" strokeWidth={1.9} />
        {/* Estrecha, la palabra se va antes que la del café: la brújula y su `title` bastan
            para algo que se mira una vez. */}
        <span className="max-[880px]:hidden">{t("Tour")}</span>
      </button>
      <span className="ms-auto flex min-w-0 items-center gap-6 max-[880px]:gap-3">
        <Buscar {...actualizacion} />
        {/* El café, como en la barra de la 0.4.7, a mano desde cualquier apartado y no solo en
            «Ayuda y winshotx». Al final y sin color: se ve, pero no pide nada. Adónde lleva va
            en el `title`, que aquí no cabe escrito. */}
        <button
          type="button"
          onClick={() => void openUrl(CAFE)}
          title={comoSeLee(CAFE)}
          className="flex shrink-0 items-center gap-1.5 text-[13px] font-semibold whitespace-nowrap text-apagado transition-colors hover:text-titulo"
        >
          <Coffee className="size-4" strokeWidth={1.9} />
          {t("Apoyar")}
        </button>
      </span>
    </footer>
  );
}

/**
 * Un grupo de la barra: su nombre en gris y sus opciones. El nombre de «Tema» y de «Idioma»
 * solo sale con la ventana estirada a 1000 o más: sus opciones se explican solas, y con el café
 * no cabían a 900 (medido: faltaban 55 px en español y 69 en inglés). El de «Al soltar» sale
 * siempre, que «Barra / Copia» solas no dicen de qué.
 */
function Grupo({ nombre, siempre = false, children }: { nombre: string; siempre?: boolean; children: React.ReactNode }) {
  return (
    <span className="flex shrink-0 items-center gap-2.5">
      <span className={`text-[13px] whitespace-nowrap text-apagado ${siempre ? "" : "max-[999px]:hidden"}`}>{nombre}</span>
      {children}
    </span>
  );
}

/**
 * Buscar versión nueva, y su respuesta. «Ya tienes la última» solo si se ha preguntado desde
 * aquí: sin preguntar (la ventana mira sola al abrirse) sería un «todo va bien» que no pide
 * nada. Con algo que decir, lo dice la cabecera y aquí no sale nada.
 *
 * La respuesta va más corta que en «Ayuda y winshotx» («estás en la última versión»): en
 * inglés, «you are on the latest version» no cabía en la barra a 900 de ancho (medido).
 */
function Buscar({ fase, mirar }: { fase: Fase; mirar: () => void }) {
  const t = useT();
  const [pedido, setPedido] = useState(false);
  if (fase.tipo === "mirando" || (fase.tipo === "aldia" && pedido)) {
    return (
      <span className="flex items-center gap-1.5 text-[13px] whitespace-nowrap text-apagado" aria-live="polite">
        {fase.tipo === "mirando" && <RefreshCw className="size-4 shrink-0 animate-spin" strokeWidth={1.9} />}
        {fase.tipo === "mirando" ? t("Buscando…") : t("Ya tienes la última")}
      </span>
    );
  }
  if (fase.tipo !== "quieto" && fase.tipo !== "aldia") return null;
  return (
    <button
      type="button"
      data-buscar
      onClick={() => {
        setPedido(true);
        mirar();
      }}
      className="flex items-center gap-1.5 text-[13px] font-semibold whitespace-nowrap text-apagado transition-colors hover:text-titulo"
    >
      <RefreshCw className="size-4" strokeWidth={1.9} />
      {t("Buscar actualizaciones")}
    </button>
  );
}
