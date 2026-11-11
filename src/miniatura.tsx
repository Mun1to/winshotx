import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { Miniaturas } from "./components/miniatura/Miniaturas";
import "./styles.css";

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <Miniaturas />
  </StrictMode>,
);
