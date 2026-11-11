import { useEffect, useState } from "react";
import { LogicalSize, getCurrentWindow } from "@tauri-apps/api/window";
import { getSettings } from "../lib/ipc";
import { aplicarTema } from "../lib/tema";
import { aplicarIdioma } from "../lib/i18n";
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
        aplicarIdioma(ajustes.language);
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
    // Cada pantalla pide el alto que necesita, y es ESTA linea la que manda: lo que dice
    // `tauri.conf.json` es solo el tamanno con el que nace la ventana, y se pisa aqui en
    // cuanto carga la aplicacion.
    //
    // Subio de 470 a 510 el 27 de agosto de 2026 al entrar el microfono, y de 510 a 620 el
    // 17 de octubre: con el anillo encendido, «Los ultimos segundos» son seis filas y la
    // seccion se cortaba a media fila. Ninguna pantalla de ajustes lleva rueda, esa es la
    // regla; si el contenido no cabe, se agranda la ventana o se compacta el contenido, y
    // aqui se han hecho las dos cosas. Y de 620 a 760 el 11 de noviembre de 2026: la tecla de
    // zoom y «La camara» dejaron la pestanna Grabar cortada por abajo.
    void ventana.setSize(new LogicalSize(840, vista === "bienvenida" ? 640 : 760));
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
