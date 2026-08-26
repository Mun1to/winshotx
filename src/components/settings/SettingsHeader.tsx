import { Check } from "lucide-react";
import { Segmented } from "../ui/Segmented";

/**
 * Las cuatro secciones de los ajustes, en el orden en que se usan.
 *
 * Sin frase explicando de que va cada una: la seccion se llama Capturar y dentro pone
 * "Al pulsar el atajo", asi que una linea diciendo "el atajo, la espera y que entra en la
 * foto" no ensennaba nada que no estuviera ya debajo, y ocupaba sitio.
 */
export const SECCIONES = [
  { value: "capturar", label: "Capturar" },
  { value: "grabar", label: "Grabar" },
  { value: "teclas", label: "Teclas de Windows" },
  { value: "app", label: "La app" },
] as const satisfies readonly { value: string; label: string }[];

export type SeccionId = (typeof SECCIONES)[number]["value"];

interface Props {
  activa: SeccionId;
  onCambiar: (id: SeccionId) => void;
  version: string;
  guardado: boolean;
  onSalir: () => void;
}

/**
 * La cabecera: la version a la izquierda, las cuatro secciones en el medio y salir a la
 * derecha.
 *
 * Sin logo ni nombre: la ventana ya se llama "winshotx · ajustes" en su propia barra de
 * titulo, dos pixeles mas arriba. Repetirlo dentro era decir lo mismo dos veces seguidas y
 * ademas empujaba las secciones a la izquierda, que es de donde vienen las ganas de
 * centrarlas.
 *
 * Es una cabecera de pagina web y no un menu lateral por una razon de espacio, no de gusto:
 * una columna a la izquierda se lleva casi doscientos pixeles de ANCHO, que es justo lo que
 * hace falta para poner los ajustes en dos columnas y que una seccion entera quepa de una
 * vez. Arriba, la navegacion cuesta alto una sola vez y el ancho queda entero para lo que se
 * ha venido a ver. Aqui vive tambien el pie que habia antes: dos barras de adorno en una
 * ventana de 520 px de alto eran una de mas.
 *
 * Las secciones son un `Segmented`, el MISMO componente que eligen los segundos de espera o
 * los fps ahi debajo. No una copia parecida: el mismo, para que no puedan separarse con el
 * tiempo. Antes eran pestannas subrayadas y eran un estilo que no existia en ningun otro
 * sitio de la aplicacion.
 */
export function SettingsHeader({ activa, onCambiar, version, guardado, onSalir }: Props) {
  // Tres columnas y las de los lados iguales: es lo unico que deja las secciones en el
  // centro DE LA VENTANA. Con una fila normal quedaban centradas en el hueco que dejaban
  // los lados, que no miden lo mismo, asi que salian corridas hacia un lado.
  return (
    <header className="grid h-12 shrink-0 grid-cols-[1fr_auto_1fr] items-center gap-3 border-b border-white/8 px-3">
      {/* Con la palabra delante y al tamanno de las explicaciones de las filas: un
          numero suelto a 11 px no se sabia que era, y habia que acercarse a leerlo. */}
      <span className="justify-self-start ps-1 text-[12.5px] text-neutral-400">
        Versión {version}
      </span>

      <nav aria-label="Secciones de ajustes" data-tour="secciones" className="justify-self-center">
        <Segmented
          ajustado
          value={activa}
          options={SECCIONES as unknown as { value: SeccionId; label: string }[]}
          onChange={onCambiar}
        />
      </nav>

      <span className="flex items-center justify-self-end gap-2">
        {/* Ocupa sitio siempre, tenga texto o no: sin esto, guardar movia el boton de
            salir cada vez que se tocaba un interruptor. */}
        <span className="flex w-[74px] items-center justify-end gap-1.5 text-[11px] text-emerald-400">
          {guardado && (
            <>
              <Check className="size-3" />
              Guardado
            </>
          )}
        </span>
        <button
          type="button"
          onClick={onSalir}
          className="rounded-md px-2 py-1 text-[11.5px] text-neutral-400 transition-colors hover:bg-red-500/15 hover:text-red-300"
        >
          Salir
        </button>
      </span>
    </header>
  );
}
