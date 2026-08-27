import { useEffect, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Copy, X } from "lucide-react";
import { copyPinned } from "../../lib/ipc";
import { useT } from "../../lib/i18n";

/**
 * Una captura dejada flotando encima de todo, para tenerla delante mientras se trabaja.
 *
 * Nace exactamente encima de donde estaba el recorte y del mismo tamanno, asi que al
 * aparecer no se mueve nada: parece que ese trozo de pantalla se ha quedado quieto. Luego
 * se arrastra con el raton a donde estorbe menos.
 *
 * **Sin barra de titulo, sin cerrar en una esquina y sin nada encima de la imagen.** Los
 * dos botones solo salen al pasar el raton por encima, y en la esquina de arriba a la
 * derecha, que es donde ya los busca todo el mundo. Una captura anclada que lleva
 * cromo alrededor deja de parecer la pantalla y empieza a parecer un visor de imagenes.
 */
export function PinWindow({ imagen }: { imagen: string }) {
  const t = useT();
  const [encima, setEncima] = useState(false);
  const [copiada, setCopiada] = useState(false);

  const cerrar = () => void getCurrentWindow().close();

  useEffect(() => {
    // Escape cierra, que es lo que hace todo lo demas de winshotx. Y Ctrl+C copia, porque
    // es la razon numero uno para volver a mirar una captura anclada.
    const alPulsar = (e: KeyboardEvent) => {
      if (e.key === "Escape") cerrar();
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "c") void copiar();
    };
    window.addEventListener("keydown", alPulsar);
    return () => window.removeEventListener("keydown", alPulsar);
  }, []);

  const copiar = async () => {
    await copyPinned(imagen);
    setCopiada(true);
    // El aviso se va solo: una captura anclada esta para mirarla, no para leer mensajes.
    setTimeout(() => setCopiada(false), 1400);
  };

  return (
    <div
      data-tauri-drag-region
      onPointerEnter={() => setEncima(true)}
      onPointerLeave={() => setEncima(false)}
      onDoubleClick={cerrar}
      title={t("Arrastra para moverla · Esc para cerrarla")}
      className="relative h-full w-full cursor-grab overflow-hidden bg-black active:cursor-grabbing"
    >
      <img
        src={convertFileSrc(imagen)}
        alt=""
        draggable={false}
        data-tauri-drag-region
        className="pointer-events-none h-full w-full object-contain"
      />

      <div
        className={`absolute end-1.5 top-1.5 flex gap-1 transition-opacity duration-150 ${
          encima ? "opacity-100" : "pointer-events-none opacity-0"
        }`}
      >
        <Boton icono={Copy} etiqueta={t("Copiar")} onClick={() => void copiar()} />
        <Boton icono={X} etiqueta={t("Cerrar")} onClick={cerrar} peligro />
      </div>

      {copiada && (
        <span className="absolute bottom-2 left-1/2 -translate-x-1/2 rounded-md bg-emerald-500/90 px-2 py-1 text-[11px] font-medium text-white shadow-lg">
          {t("Copiada")}
        </span>
      )}
    </div>
  );
}

function Boton({
  icono: Icono,
  etiqueta,
  onClick,
  peligro = false,
}: {
  icono: typeof Copy;
  etiqueta: string;
  onClick: () => void;
  peligro?: boolean;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      title={etiqueta}
      aria-label={etiqueta}
      className={`flex size-6 items-center justify-center rounded-md bg-neutral-900/80 text-neutral-200 backdrop-blur-sm transition-colors ${
        peligro ? "hover:bg-red-500 hover:text-white" : "hover:bg-neutral-700"
      }`}
    >
      <Icono className="size-3.5" />
    </button>
  );
}
