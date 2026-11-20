import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { getSettings } from "../lib/ipc";
import { aplicarTema } from "../lib/tema";
import { aplicarIdioma, idiomaActivo, useT } from "../lib/i18n";
import { type Tamanno, useVentanaFija } from "../lib/ventanaFija";
import { SettingsApp } from "./settings/SettingsApp";
import { WelcomeApp } from "./welcome/WelcomeApp";

/**
 * La ventana principal es la misma para las dos cosas: la bienvenida de la primera vez
 * y los ajustes de siempre. Una ventana menos que crear, posicionar y cerrar, y el paso
 * de una a otra no parpadea.
 */
type Vista = "cargando" | "bienvenida" | "ajustes";

export function App() {
  const t = useT();
  const idioma = idiomaActivo();
  const [vista, setVista] = useState<Vista>("cargando");
  // Solo cuando se acaba de terminar la bienvenida. Quien abre los ajustes de siempre no
  // quiere un tour delante: para eso esta el boton de repetirlo en "La app".
  const [tourAlEntrar, setTourAlEntrar] = useState(false);
  // El tamaño que se le dio a mano la última vez, si se le dio (S40).
  const [guardada, setGuardada] = useState<Tamanno | null>(null);

  // Siempre el mismo tamaño, salvo que la estire quien la usa (`ventanaFija.ts`). Va aquí, que
  // es quien tiene la ventana: la bienvenida y los ajustes se turnan dentro sin tocarla.
  useVentanaFija(vista !== "cargando", guardada);

  useEffect(() => {
    void getSettings()
      // Si los ajustes no se pueden leer, mejor los ajustes que una bienvenida repetida.
      .then((ajustes) => {
        // El modulo del tema ya ha pintado con lo que se recordaba del arranque anterior.
        // Esto solo lo confirma o lo corrige, que es lo que hace falta la primera vez en
        // una maquina nueva o si alguien edito el archivo de ajustes a mano.
        aplicarTema(ajustes.theme);
        aplicarIdioma(ajustes.language);
        setGuardada(ajustes.ventanaAjustes ?? null);
        setVista(ajustes.onboarded ? "ajustes" : "bienvenida");
      })
      .catch(() => setVista("ajustes"));
  }, []);

  useEffect(() => {
    if (vista === "cargando") return;
    const ventana = getCurrentWindow();
    void ventana.setTitle(`winshotx · ${vista === "bienvenida" ? t("Bienvenida") : t("Ajustes")}`);
    // Ninguna de las dos pide aqui su tamanno: la ventana mide siempre lo mismo
    // (`useVentanaFija`, arriba). Historia, para no volver atras: hasta la 0.4 era un alto fijo
    // que subia con cada fila nueva (470, 510, 620, 760); de S24 a la 0.5.0 media lo que cada
    // pestanna o paso, y crecia y encogia al pulsar el riel, con el historial estirandola hasta
    // 760. Munito, el 15-nov-2026: «el tamaño siempre el mismo, a no ser que lo modifiques tú».
    // Desde S40 es el de Grabar para todos, lo que no cabe baja con la rueda dentro, y lo que
    // sobra lo llena el dibujo de la derecha.
  }, [vista, idioma, t]);

  if (vista === "cargando") return <div className="h-full bg-lienzo" />;
  if (vista === "bienvenida") {
    return (
      <WelcomeApp
        onDone={() => {
          setTourAlEntrar(true);
          setVista("ajustes");
        }}
      />
    );
  }
  return (
    <SettingsApp
      onVerBienvenida={() => setVista("bienvenida")}
      arrancarTour={tourAlEntrar}
    />
  );
}
