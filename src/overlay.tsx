import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { SelectionCanvas } from "./components/overlay/SelectionCanvas";
import "./styles.css";

// Cada monitor abre su propia ventana overlay: ?monitor=<indice>
const monitorId = Number(new URLSearchParams(window.location.search).get("monitor") ?? 0);

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <SelectionCanvas monitorId={monitorId} />
  </StrictMode>,
);
