import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { EditorApp } from "./components/editor/EditorApp";
import "./styles.css";

const sessionId = new URLSearchParams(window.location.search).get("session") ?? "";

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <EditorApp sessionId={sessionId} />
  </StrictMode>,
);
