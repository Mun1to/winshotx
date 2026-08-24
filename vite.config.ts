import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

// La version se escribe UNA vez, en package.json, y de ahi baja al pie de los ajustes.
const paquete = JSON.parse(readFileSync(resolve(__dirname, "package.json"), "utf8"));

// Tres ventanas, tres entradas HTML: ajustes, overlay de seleccion y editor.
export default defineConfig({
  plugins: [react(), tailwindcss()],
  define: { __VERSION__: JSON.stringify(paquete.version) },
  clearScreen: false,
  server: {
    port: 1421,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  build: {
    target: "chrome110",
    rollupOptions: {
      input: {
        main: resolve(__dirname, "index.html"),
        overlay: resolve(__dirname, "overlay.html"),
        editor: resolve(__dirname, "editor.html"),
        recorder: resolve(__dirname, "recorder.html"),
      },
    },
  },
});
