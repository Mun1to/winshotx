import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { BarraScroll } from "./components/overlay/BarraScroll";
import { SelectionCanvas } from "./components/overlay/SelectionCanvas";
import { PARAMETRO_BARRA } from "./lib/scroll";
import "./styles.css";

const parametros = new URLSearchParams(window.location.search);
// Cada monitor abre su propia ventana overlay: ?monitor=<indice>
const monitorId = Number(parametros.get("monitor") ?? 0);

// La barrita de la captura con scroll vive en esta misma página (`?barra=scroll`): es una
// pieza del overlay, y así no hace falta otra entrada que compilar.
const esBarraScroll = parametros.get(PARAMETRO_BARRA) === "scroll";

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    {esBarraScroll ? <BarraScroll /> : <SelectionCanvas monitorId={monitorId} />}
  </StrictMode>,
);
