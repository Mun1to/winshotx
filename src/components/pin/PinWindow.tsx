import { useEffect, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Copy, Download, Type, X } from "lucide-react";
import { copyPinned, pinnedText, savePinned } from "../../lib/ipc";
import { useT } from "../../lib/i18n";

/**
 * Una captura dejada flotando encima de todo, para tenerla delante mientras se trabaja.
 *
 * Nace exactamente encima de donde estaba el recorte y del mismo tamanno, asi que al
 * aparecer no se mueve nada: parece que ese trozo de pantalla se ha quedado quieto. Luego
 * se arrastra con el raton a donde estorbe menos.
 *
 * **Sin barra de titulo, sin cerrar en una esquina y sin nada encima de la imagen.** Los
 * botones solo salen al pasar el raton por encima, y en la esquina de arriba a la
 * derecha, que es donde ya los busca todo el mundo. Una captura anclada que lleva
 * cromo alrededor deja de parecer la pantalla y empieza a parecer un visor de imagenes.
 *
 * **Las acciones y sus teclas son las mismas que en la barra de captura**, porque es la
 * misma imagen: copiar, guardar y leer el texto. Aprender que aqui `T` hace otra cosa
 * seria aprender dos programas.
 */
export function PinWindow({ imagen }: { imagen: string }) {
  const t = useT();
  const [encima, setEncima] = useState(false);
  const [aviso, setAviso] = useState<{ texto: string; malo: boolean } | null>(null);

  const cerrar = () => void getCurrentWindow().close();

  /**
   * Un solo sitio para las tres acciones, porque las tres terminan igual: un aviso que se
   * va solo. Una captura anclada esta para mirarla, no para leer mensajes.
   */
  const hacer = async (accion: () => Promise<unknown>, bien: string) => {
    try {
      await accion();
      setAviso({ texto: bien, malo: false });
    } catch (e) {
      // Los errores llegan de Rust escritos en espannol, y su texto es la clave.
      setAviso({ texto: String(e), malo: true });
    }
  };

  const copiar = () => hacer(() => copyPinned(imagen), "Copiada");
  const guardar = () => hacer(() => savePinned(imagen), "Guardada");
  const leer = () => hacer(() => pinnedText(imagen), "Texto copiado");

  useEffect(() => {
    if (!aviso) return;
    // El malo dura mas: si algo ha salido mal hay que llegar a leerlo.
    const id = setTimeout(() => setAviso(null), aviso.malo ? 3000 : 1400);
    return () => clearTimeout(id);
  }, [aviso]);

  useEffect(() => {
    // Escape cierra, que es lo que hace todo lo demas de winshotx. Las otras tres son las
    // mismas teclas que en la barra de captura.
    const alPulsar = (e: KeyboardEvent) => {
      const tecla = e.key.toLowerCase();
      if (e.key === "Escape") {
        cerrar();
      } else if ((e.ctrlKey || e.metaKey) && tecla === "c") {
        void copiar();
      } else if ((e.ctrlKey || e.metaKey) && tecla === "s") {
        // Sin esto, el navegador que hay debajo abriria su cuadro de guardar la pagina.
        e.preventDefault();
        void guardar();
      } else if (tecla === "t" && !e.ctrlKey && !e.metaKey && !e.altKey) {
        void leer();
      }
    };
    window.addEventListener("keydown", alPulsar);
    return () => window.removeEventListener("keydown", alPulsar);
  }, [imagen]);

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
        <Boton icono={Copy} etiqueta={`${t("Copiar")} · Ctrl+C`} onClick={() => void copiar()} />
        <Boton
          icono={Download}
          etiqueta={`${t("Guardar")} · Ctrl+S`}
          onClick={() => void guardar()}
        />
        <Boton icono={Type} etiqueta={`${t("Copiar el texto")} · T`} onClick={() => void leer()} />
        <Boton icono={X} etiqueta={`${t("Cerrar")} · Esc`} onClick={cerrar} peligro />
      </div>

      {aviso && (
        <span
          role="status"
          className={`absolute bottom-2 left-1/2 max-w-[90%] -translate-x-1/2 truncate rounded-md px-2 py-1 text-[11px] font-medium text-white shadow-lg ${
            aviso.malo ? "bg-red-500/90" : "bg-emerald-500/90"
          }`}
        >
          {t(aviso.texto)}
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
