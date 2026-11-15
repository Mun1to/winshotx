import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
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
    // Ninguna de las dos pide aqui su tamanno: miden lo que mide lo que ensennan
    // (`useVentanaALaMedida`). Los ajustes desde S24 (14-nov-2026), porque con un alto fijo
    // para todas, el de la mas larga, Capturar abria con 160 px vacios debajo; la bienvenida
    // desde S38 (15-nov-2026), que con 840 x 640 fijos dejaba entre 179 y 260 px vacios bajo
    // cada paso. Lo que dice `tauri.conf.json` es solo el tamanno con el que nace la ventana,
    // y el ancho es el suyo: la bienvenida no lo toca.
    //
    // Historia del alto fijo, que explica la regla que sigue en pie: subio de 470 a 510 el
    // 27 de agosto de 2026 al entrar el microfono, a 620 el 17 de octubre (el anillo) y a 760
    // el 11 de noviembre (la tecla de zoom y la camara). Ninguna pantalla de ajustes lleva
    // rueda: si el contenido no cabe, se agranda la ventana.
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
