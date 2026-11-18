import { Fragment, useEffect, useRef, useState } from "react";
import { corriendoEnLinux, partesDeAtajo } from "../../lib/teclas";
import { useT } from "../../lib/i18n";

/** Teclas que solo acompañan: por sí solas no forman un atajo. */
const MODIFIERS = new Set([
  "Control",
  "Shift",
  "Alt",
  "Meta",
  "ControlLeft",
  "ControlRight",
  "ShiftLeft",
  "ShiftRight",
  "AltLeft",
  "AltRight",
  "MetaLeft",
  "MetaRight",
]);

/**
 * La tecla del logo en WebKitGTK: se llama `Super`, su código es `OSLeft` u `OSRight` y `metaKey`
 * no se enciende nunca, ni en X11 ni en Wayland. Sin mirar esto, Super+Mayús+K se guardaba como
 * Mayús+K, que es otro atajo.
 */
const esSuperDeWebKit = (e: KeyboardEvent) => e.key === "Super" || e.code === "OSLeft" || e.code === "OSRight";

interface Props {
  value: string;
  onChange: (value: string) => void;
  /** false cuando el sistema no ha dejado registrarlo. */
  active: boolean;
}

export function ShortcutField({ value, onChange, active }: Props) {
  const t = useT();
  const [recording, setRecording] = useState(false);
  const boxRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (!recording) return;

    // Las Super que siguen apretadas, porque el evento de la letra no lo cuenta en WebKitGTK.
    const superPulsadas = new Set<string>();

    const onKey = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();

      if (e.key === "Escape") {
        setRecording(false);
        return;
      }
      if (esSuperDeWebKit(e)) {
        superPulsadas.add(e.code);
        return;
      }
      if (MODIFIERS.has(e.code) || MODIFIERS.has(e.key)) return;

      const parts: string[] = [];
      // La tecla Windows es Super, no Ctrl. Estaban juntas en la misma línea, así que
      // pulsar Win+X guardaba un atajo de Ctrl+X: otra tecla y otro atajo.
      if (e.ctrlKey) parts.push("CmdOrCtrl");
      if (e.metaKey || superPulsadas.size > 0) parts.push("Super");
      if (e.shiftKey) parts.push("Shift");
      if (e.altKey) parts.push("Alt");
      // Un atajo global sin modificadores secuestraría la tecla en todo el sistema.
      if (parts.length === 0) return;
      // Sin código físico, el atajo saldría como "CmdOrCtrl+Shift+" y Windows no puede
      // registrar eso: pasa con el teclado en pantalla y con las teclas automatizadas.
      if (!e.code) return;

      parts.push(e.code);
      // Cerrar ANTES de guardar. Si el guardado se va por el desagüe, el campo se
      // quedaba pidiendo teclas para siempre y parecía que no se había enterado.
      setRecording(false);
      onChange(parts.join("+"));
    };

    const onKeyUp = (e: KeyboardEvent) => {
      if (esSuperDeWebKit(e)) superPulsadas.delete(e.code);
    };
    // Super sola abre las actividades de GNOME: la ventana pierde el foco y el keyup no llega.
    const onBlur = () => superPulsadas.clear();

    window.addEventListener("keydown", onKey, true);
    window.addEventListener("keyup", onKeyUp, true);
    window.addEventListener("blur", onBlur);
    return () => {
      window.removeEventListener("keydown", onKey, true);
      window.removeEventListener("keyup", onKeyUp, true);
      window.removeEventListener("blur", onBlur);
    };
  }, [recording, onChange]);

  useEffect(() => {
    if (!recording) return;
    const onClickOutside = (e: MouseEvent) => {
      if (!boxRef.current?.contains(e.target as Node)) setRecording(false);
    };
    window.addEventListener("mousedown", onClickOutside);
    return () => window.removeEventListener("mousedown", onClickOutside);
  }, [recording]);

  const parts = partesDeAtajo(value);

  // Sin caja y sin teclas en cajitas desde S40: «Ctrl + Shift + S» como texto, que es como se
  // dice un atajo. Lo que dice «esto se cambia aquí» es la raya azul de debajo, al pasar el
  // ratón, con el foco del teclado y mientras escucha las teclas. Munito, con la 0.5.0
  // delante: «no me gusta el diseño de esto» (un recuadro con «Win» «Shift» «S» en cajitas).
  const tinta = active ? "text-titulo" : "text-[color:light-dark(#b91c1c,#f87171)]";
  return (
    <button
      ref={boxRef}
      type="button"
      onClick={() => setRecording((r) => !r)}
      title={
        !recording
          ? t("Clic para cambiar el atajo")
          : corriendoEnLinux()
            ? t("Pulsa la combinación · las que el escritorio se reserva no llegan hasta aquí")
            : t("Pulsa la combinación · las que Windows se reserva no llegan hasta aquí")
      }
      data-atajo
      className="group flex h-9 items-center justify-end px-1 outline-none"
    >
      <span
        className={`flex items-center gap-1.5 border-b-2 pb-0.5 text-[14px] leading-tight whitespace-nowrap transition-colors duration-150 ${
          recording
            ? "border-marca"
            : "border-transparent group-hover:border-marca group-focus-visible:border-marca"
        }`}
      >
        {recording ? (
          <span className="animate-pulse font-medium text-marca">{t("Pulsa las teclas…")}</span>
        ) : (
          parts.map((part, index) => (
            <Fragment key={`${part}-${index}`}>
              {index > 0 && (
                <span aria-hidden="true" className="text-apagado">
                  +
                </span>
              )}
              <span className={`font-semibold ${tinta}`}>{part}</span>
            </Fragment>
          ))
        )}
      </span>
    </button>
  );
}
