import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { Countdown } from "./components/countdown/Countdown";
import "./styles.css";

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <Countdown />
  </StrictMode>,
);
