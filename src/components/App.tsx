import { useEffect, useState } from "react";
import { LogicalSize, getCurrentWindow } from "@tauri-apps/api/window";
import { getSettings } from "../lib/ipc";
import { aplicarTema } from "../lib/tema";
import { SettingsApp } from "./settings/SettingsApp";
import { WelcomeApp } from "./welcome/WelcomeApp";

/**
 * La ventana principal es la misma para las dos cosas: la bienvenida de la primera vez
 * y los ajustes de siempre. Una ventana menos que crear, posicionar y cerrar, y el paso
 * de una a otra no parpadea.
 */
type Vista = "cargando" | "bienvenida" | "ajustes";

export function App() {
  const [vista, setVista] = useState<Vista>("cargando");
  // Solo cuando se acaba de terminar la bienvenida. Quien abre los ajustes de siempre no
  // quiere un tour delante: para eso esta el boton de repetirlo en "La app".
  const [tourAlEntrar, setTourAlEntrar] = useState(false);

  useEffect(() => {
    void getSettings()
      // Si los ajustes no se pueden leer, mejor los ajustes que una bienvenida repetida.
      .then((ajustes) => {
        // El modulo del tema ya ha pintado con lo que se recordaba del arranque anterior.
        // Esto solo lo confirma o lo corrige, que es lo que hace falta la primera vez en
        // una maquina nueva o si alguien edito el archivo de ajustes a mano.
        aplicarTema(ajustes.theme);
        setVista(ajustes.onboarded ? "ajustes" : "bienvenida");
      })
      .catch(() => setVista("ajustes"));
  }, []);

  useEffect(() => {
    if (vista === "cargando") return;
    const ventana = getCurrentWindow();
    void ventana.setTitle(
      vista === "bienvenida" ? "winshotx · bienvenida" : "winshotx · ajustes",
    );
    // Cada pantalla pide el alto que necesita. La bienvenida es un texto largo y necesita
    // 640; los ajustes, con la navegacion arriba y los bloques en dos columnas, caben en
    // 470. Dejar la ventana siempre en el alto de la mas alta le pone a los ajustes un
    // palmo de hueco debajo, y es la que se abre todos los dias.
    void ventana.setSize(new LogicalSize(840, vista === "bienvenida" ? 640 : 470));
  }, [vista]);

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
