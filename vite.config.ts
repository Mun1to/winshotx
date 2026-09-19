/// <reference types="vitest/config" />
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

// La version se escribe UNA vez, en package.json, y de ahi baja al pie de los ajustes.
const paquete = JSON.parse(readFileSync(resolve(__dirname, "package.json"), "utf8"));

// Una entrada HTML por ventana: ajustes, overlay de seleccion, editor, barra de
// grabacion y la cuenta atras del temporizador.
export default defineConfig({
  plugins: [react(), tailwindcss()],
  define: { __VERSION__: JSON.stringify(paquete.version) },
  clearScreen: false,
  server: {
    port: 1421,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  // El banco de pruebas del frontend. Corre con el MISMO Vite que compila la aplicacion,
  // asi que lo que pasa una prueba se comporta igual dentro de la ventana: mismos alias,
  // mismo `__VERSION__`, mismo Tailwind. Un empaquetador aparte se habria desincronizado.
  test: {
    environment: "happy-dom",
    globals: true,
    setupFiles: ["./src/test/preparar.ts"],
    include: ["src/**/*.test.{ts,tsx}"],
  },
  build: {
    target: "chrome110",
    rollupOptions: {
      input: {
        main: resolve(__dirname, "index.html"),
        overlay: resolve(__dirname, "overlay.html"),
        editor: resolve(__dirname, "editor.html"),
        recorder: resolve(__dirname, "recorder.html"),
        cuenta: resolve(__dirname, "cuenta.html"),
        pin: resolve(__dirname, "pin.html"),
        "tray-menu": resolve(__dirname, "tray-menu.html"),
      },
    },
  },
});
