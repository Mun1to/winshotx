/**
 * Las filas de Ajustes de la grabación nueva (S2): la tecla de zoom y la cámara.
 *
 * Van aparte de `SettingsApp.tsx` para que cada pieza del plan viva en su archivo; allí
 * solo se ponen en su sitio, con el mismo `settings` y el mismo `patch` que las demás.
 */
import { Webcam, ZoomIn } from "lucide-react";
import type { CamaraConectada } from "../../lib/ipc";
import { useT } from "../../lib/i18n";
import { chocaCon } from "../../lib/teclaZoom";
import type { Settings } from "../../lib/types";
import { Switch } from "../ui/Switch";
import { Row, RowButton, Section } from "./Section";
import { ShortcutField } from "./ShortcutField";

interface Props {
  settings: Settings;
  patch: (cambio: Partial<Settings>) => void;
}

/** La tecla que, mientras grabas, deja un zoom donde tienes el ratón. */
export function FilaTeclaZoom({ settings, patch }: Props) {
  const t = useT();
  const choque = chocaCon(settings);
  const aviso = {
    record: t("es la de grabar: al pulsarla pararías la grabación"),
    capture: t("es la de capturar: al pulsarla abrirías una captura"),
    replay: t("es la de los últimos segundos"),
    edit: t("es la de capturar y editar"),
  };
  return (
    <Row
      icon={<ZoomIn className="size-4" />}
      label={t("Tecla de zoom")}
      explicacion={t(
        "Mientras grabas, púlsala y la cámara se acercará a donde tengas el ratón en ese momento. Sale como un zoom en la línea de tiempo del editor, que luego puedes mover o quitar. La pulsación también le llega al programa que estás grabando: por eso viene una que casi ninguno usa.",
      )}
      hint={choque ? aviso[choque] : t("acerca el vídeo al ratón")}
      tone={choque ? "warn" : "normal"}
      control={
        <ShortcutField
          value={settings.zoomShortcut}
          active={choque === null}
          onChange={(v) => patch({ zoomShortcut: v })}
        />
      }
    />
  );
}

/** Grabar también la webcam y, si hay más de una, cuál. */
export function SeccionCamara({ settings, patch, camaras }: Props & { camaras: CamaraConectada[] }) {
  const t = useT();
  // Si la guardada ya no está, se graba la primera: es lo que hace Rust.
  const elegida = camaras.find((c) => c.id === settings.cameraDevice) ?? camaras[0];
  const sinCamara = camaras.length === 0;
  return (
    <Section title={t("La cámara")}>
      <Row
        icon={<Webcam className="size-4" />}
        label={t("Grabar también la cámara")}
        explicacion={t(
          "Graba tu webcam a la vez, en su propio archivo junto al vídeo, para ponerla después en una esquina desde el editor. La luz de la cámara se enciende solo mientras grabas. Si otro programa la está usando o Windows no la deja (Privacidad, Cámara), se graba sin ella.",
        )}
        hint={
          sinCamara
            ? t("no hay ninguna cámara conectada")
            : settings.recordCamera && elegida && camaras.length === 1
              ? elegida.nombre
              : t("tu cara en su propio archivo")
        }
        tone={sinCamara && settings.recordCamera ? "warn" : "normal"}
        control={
          <Switch
            checked={settings.recordCamera}
            onChange={(v) => patch({ recordCamera: v })}
            label={t("Grabar también la cámara")}
          />
        }
      />
      {/* Elegir entre una sola es una fila de ruido, igual que con las pantallas. Con
          varias, una fila con la que se graba y un botón que pasa a la siguiente: los
          nombres de las cámaras son largos («Logitech HD Pro Webcam C920») y en botones
          uno al lado del otro salían cortados, y la ventana no tiene alto para apilarlos. */}
      {settings.recordCamera && camaras.length > 1 && elegida && (
        <Row
          icon={<Webcam className="size-4" />}
          label={t("Qué cámara")}
          explicacion={t("Se graba la que elijas. Si un día no está conectada, se usa la primera que haya.")}
          hint={elegida.nombre}
          control={
            <RowButton
              onClick={() => {
                const siguiente = camaras[(camaras.indexOf(elegida) + 1) % camaras.length];
                patch({ cameraDevice: siguiente.id });
              }}
            >
              {t("Cambiar")}
            </RowButton>
          }
        />
      )}
    </Section>
  );
}
