import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { RecorderBar } from "./components/recording/RecorderBar";
import "./styles.css";

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <RecorderBar />
  </StrictMode>,
);
