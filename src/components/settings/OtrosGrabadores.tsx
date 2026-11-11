import { useCallback, useEffect, useState } from "react";
import { Info } from "lucide-react";
import { abrirCapturasDeWindows, otrosGrabadores, type OtrosGrabadoresVistos } from "../../lib/ipc";
import { useT } from "../../lib/i18n";
import { Row, RowButton } from "./Section";

/**
 * Avisa de los otros programas que graban la pantalla todo el rato, debajo de «Grabar
 * siempre lo último», que hace lo mismo.
 *
 * Munir, 11-nov-2026: *«un botón de inhabilitar capturador de NVIDIA, porque consume todo
 * el rato y no es necesario si tienes winshotx ya»*. No se apagan desde aquí (los ajustes
 * de NVIDIA no son públicos; ver `src-tauri/src/otros_grabadores.rs`): se dice cómo.
 *
 * - **La Barra de juego** solo sale si de verdad graba, y su botón abre la página de
 *   Windows donde se apaga.
 * - **NVIDIA** sale con su panel en marcha, que es lo único que se ve desde fuera, y por
 *   eso lleva «Ya lo tengo apagado»: quien ya la apagó no tiene por qué verlo cada vez.
 */
const NVIDIA_YA_APAGADO = "winshotx.nvidiaYaApagado";

function yaApagado(): boolean {
  try {
    return window.localStorage.getItem(NVIDIA_YA_APAGADO) === "1";
  } catch {
    return false;
  }
}

export function OtrosGrabadores() {
  const t = useT();
  const [visto, setVisto] = useState<OtrosGrabadoresVistos | null>(null);
  const [nvidiaCallado, setNvidiaCallado] = useState(yaApagado);

  const mirar = useCallback(() => {
    void otrosGrabadores()
      .then(setVisto)
      .catch(() => setVisto(null));
  }, []);

  // Y otra vez al volver a la ventana: lo normal es irse a apagarlo y volver a mirar.
  useEffect(() => {
    mirar();
    window.addEventListener("focus", mirar);
    return () => window.removeEventListener("focus", mirar);
  }, [mirar]);

  const callarNvidia = () => {
    try {
      window.localStorage.setItem(NVIDIA_YA_APAGADO, "1");
    } catch {
      // Sin almacenamiento se calla igual hasta que se cierre la ventana.
    }
    setNvidiaCallado(true);
  };

  if (!visto) return null;
  return (
    <>
      {visto.barraDeJuego && (
        <Row
          icon={<Info className="size-4" />}
          label={t("La Barra de juego también graba")}
          hint={t("Windows graba en segundo plano lo mismo que esto, y gastan los dos a la vez.")}
          tone="warn"
          control={<RowButton onClick={() => void abrirCapturasDeWindows()}>{t("Apagarla")}</RowButton>}
        />
      )}
      {visto.nvidia && !nvidiaCallado && (
        <Row
          icon={<Info className="size-4" />}
          label={t("¿NVIDIA también graba?")}
          hint={t("Apaga su Repetición instantánea con Alt+Z")}
          explicacion={t("La Repetición instantánea de NVIDIA graba la pantalla todo el rato para poder guardar lo último, igual que «Grabar siempre lo último». Con las dos encendidas se paga dos veces por lo mismo. winshotx no la apaga por su cuenta: los ajustes de NVIDIA no son públicos, y tocarlos a ciegas puede romperla en su próxima actualización.")}
          tone="warn"
          control={<RowButton onClick={callarNvidia}>{t("Ya lo tengo apagado")}</RowButton>}
        />
      )}
    </>
  );
}
