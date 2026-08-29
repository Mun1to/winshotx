import { Compass, Monitor, Moon, Sun } from "lucide-react";
import { Segmented } from "../ui/Segmented";
import { aplicarIdioma, useT } from "../../lib/i18n";
import { aplicarTema } from "../../lib/tema";
import type { CaptureFlow, Language, Settings, Theme } from "../../lib/types";
import { EstadoActualizacion, useActualizacion } from "./Actualizacion";

interface Props {
  settings: Settings;
  patch: (cambio: Partial<Settings>) => void;
  version: string;
  recienActualizado: boolean;
  onTour: () => void;
}

/**
 * La barra de abajo: lo que se toca a menudo, a mano desde cualquier sección.
 *
 * Hubo un pie antes y se quitó, con razón: no llevaba nada dentro y eran dos barras de
 * adorno en una ventana pequeña. Este se gana el sitio porque **hace** cosas: el tema, el
 * idioma, qué pasa al soltar el ratón, el tour, y el estado de las actualizaciones, que es
 * lo primero que se mira al abrir los ajustes y estaba escondido en la cuarta sección.
 *
 * Los mismos ajustes siguen estando en su sección, con su nombre y su explicación entera.
 * Esto es el atajo, no el sitio donde viven: es como el menú de la bandeja de VoCript, que
 * repite el modelo y el idioma que también están en sus ajustes.
 */
export function BarraRapida({ settings, patch, version, recienActualizado, onTour }: Props) {
  const t = useT();
  const { fase, mirar, instalar, texto, alDia } = useActualizacion(version, recienActualizado);

  const temas: { value: Theme; label: string; icono: React.ReactNode }[] = [
    { value: "sistema", label: t("Automático"), icono: <Monitor className="size-3.5" /> },
    { value: "claro", label: t("Claro"), icono: <Sun className="size-3.5" /> },
    { value: "oscuro", label: t("Oscuro"), icono: <Moon className="size-3.5" /> },
  ];

  const idiomas: { value: Language; label: string }[] = [
    { value: "sistema", label: t("Automático") },
    { value: "es", label: "ES" },
    { value: "en", label: "EN" },
  ];

  // Lo que pasa al soltar el ratón, que es el ajuste que más se cambia de todos: se
  // enciende la barra para elegir, o se apaga cuando durante un rato solo se quiere copiar.
  //
  // Con palabras y no con dos iconos: una ventanita y un portapapeles dibujados en 14 px
  // no dicen «barra» ni «copia» a nadie que no lo sepa ya, y aquí sobra sitio.
  const soltar: { value: CaptureFlow; label: string }[] = [
    { value: "toolbar", label: t("Barra") },
    { value: "instant", label: t("Copia") },
  ];

  return (
    <footer className="flex h-11 shrink-0 items-center gap-2.5 border-t border-linea px-3">
      <Segmented
        compacto
        ajustado
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
      <Segmented
        compacto
        ajustado
        etiqueta={t("Idioma")}
        value={settings.language}
        options={idiomas}
        onChange={(v) => {
          aplicarIdioma(v);
          patch({ language: v });
        }}
      />

      <span className="h-5 w-px bg-linea" />

      <span className="text-[11px] text-tenue">{t("Al soltar")}</span>
      <Segmented
        compacto
        ajustado
        etiqueta={t("Al soltar el ratón")}
        value={settings.captureFlow}
        options={soltar}
        onChange={(v) => patch({ captureFlow: v })}
      />
      <button
        type="button"
        onClick={onTour}
        title={t("Tour de los ajustes")}
        aria-label={t("Tour de los ajustes")}
        className="flex items-center gap-1.5 rounded-lg px-2 py-1 text-[11px] text-apagado transition-colors hover:bg-realce hover:text-titulo"
      >
        <Compass className="size-3.5" /> {t("Tour")}
      </button>

      {/* Lo de actualizar vive aquí y ya no en la sección «La app»: es un estado que hay
          que poder mirar de un vistazo, no un ajuste que se va a buscar. */}
      <span className="ms-auto flex min-w-0 items-center">
        <EstadoActualizacion
          fase={fase}
          texto={texto}
          alDia={alDia}
          instalar={instalar}
          mirar={() => void mirar(true)}
        />
      </span>
    </footer>
  );
}
