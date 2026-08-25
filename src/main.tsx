import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { MotionConfig } from "framer-motion";
import { App } from "./components/App";
import "./styles.css";

// Quien tenga puesto "reducir movimiento" en Windows ve las pantallas quietas: framer solo
// hace caso de esa preferencia si se le dice, y por defecto anima igual.
createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <MotionConfig reducedMotion="user">
      <App />
    </MotionConfig>
  </StrictMode>,
);
