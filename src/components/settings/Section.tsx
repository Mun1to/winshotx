import { createContext, useContext, useEffect, useRef, type ReactNode } from "react";

import { Ayuda } from "./Ayuda";

interface GrupoProps {
  title?: string;
  children: ReactNode;
  /** Enganche para que el tour guiado pueda iluminar este bloque. */
  tour?: string;
}

/**
 * Un grupo de ajustes: el título suelto y las filas con una línea fina entre una y otra.
 *
 * Sin tarjeta desde la 0.5 (S36, la variación 9 que eligió Munito): con cuatro tarjetas
 * por pestaña la pantalla se leía como cajas dentro de la ventana, que es lo que no quiere
 * («nada de cárceles»). Ahora es como una lista de la web: el título en la letra de los
 * titulares y las filas sueltas.
 */
export function Grupo({ title, children, tour }: GrupoProps) {
  return (
    <section data-tour={tour} className="mb-7 last:mb-0">
      {title && <h2 className="mb-1 text-[15px] font-bold text-titulo">{title}</h2>}
      {/* La linea va entre filas y nunca encima de la primera. Se pone sobre los hijos
          directos y no sobre `.fila + .fila`: alguna fila llega envuelta en su propio
          contenedor, y una regla de hermanos adyacentes se saltaba justo esas. */}
      <div className="border-y border-linea [&>*+*]:border-t [&>*+*]:border-linea">{children}</div>
    </section>
  );
}

/**
 * Qué dice la línea de debajo del nombre.
 *
 * - `dato`: algo que hace falta ver (la carpeta, lo que ocupa la caché, de dónde graba el
 *   anillo). En gris, porque informa y no pide nada.
 * - `aviso`: algo que hay que mirar (la combinación está ocupada, cuesta Win+S). En ámbar.
 * - `error`: algo que ha fallado. En rojo.
 *
 * No hay tono verde: un «todo va bien» no pide nada y sobra (los gustos de Munito). Y la
 * explicación del ajuste ya no va aquí: la cuenta el dibujo de la derecha, y la larga sigue
 * en el icono.
 */
export type RowTone = "dato" | "aviso" | "error";

const TONO: Record<RowTone, string> = {
  dato: "text-apagado",
  aviso: "text-aviso",
  // Un rojo por tema, para que se lea en los dos (AA): el oscuro sobre claro y el claro sobre oscuro.
  error: "text-[color:light-dark(#b91c1c,#f87171)]",
};

/**
 * La fila que hay que iluminar, la que ha elegido el buscador. Una sola a la vez.
 *
 * Va por contexto y no por props porque las filas viven en varios archivos (las de grabar,
 * la del fondo, la de la miniatura) y el buscador está en la cabecera.
 */
export const FilaEncendida = createContext<string | null>(null);

interface RowProps {
  label: string;
  hint?: ReactNode;
  icon?: ReactNode;
  /**
   * La explicación larga del ajuste, la que sale al dejar el ratón sobre el icono.
   *
   * En la fila caben un nombre y una línea; el porqué no cabe, y es justo lo que hace
   * falta para decidir. Va colgada del icono porque es el único sitio de la fila que no
   * hace nada: el nombre no se pulsa y el control sí.
   */
  explicacion?: string;
  control: ReactNode;
  /** Pone el control debajo, a lo ancho, para sliders o selectores. */
  stacked?: boolean;
  tone?: RowTone;
  /** El nombre de la fila para el buscador (`mapaDeAjustes.ts`): con él, el buscador la encuentra y la ilumina. */
  id?: string;
}

export function Row({
  label,
  hint,
  icon,
  explicacion,
  control,
  stacked = false,
  tone = "dato",
  id,
}: RowProps) {
  const encendida = useContext(FilaEncendida);
  const yo = useRef<HTMLDivElement>(null);
  const luce = id !== undefined && encendida === id;

  // Al saltar desde el buscador, la fila se trae a la vista: en un apartado alto puede
  // quedar por debajo del borde y la luz no se vería.
  useEffect(() => {
    if (luce) yo.current?.scrollIntoView?.({ block: "nearest" });
  }, [luce]);

  // El hueco del icono se reserva siempre, tenga icono o no: sin eso, una fila sin
  // icono dejaba su texto cuatro pixeles a la izquierda y la columna salía torcida.
  const texto = (
    <span className="flex min-w-0 items-center gap-3">
      {explicacion ? (
        <Ayuda texto={explicacion}>{icon}</Ayuda>
      ) : (
        <span className="flex w-4 shrink-0 justify-center text-tenue">{icon}</span>
      )}
      {/* El nombre entero, sin cortar: a 15 px y con la ventana de 900, cabe; y si en otro
          idioma no cabe, baja de línea antes que perder letras. */}
      <span className="min-w-0">
        <span className="block text-pretty text-[15px] font-medium text-titulo">{label}</span>
        {/* Un dato largo sin espacios (la ruta de la carpeta) se parte donde haga falta antes
            que salirse de la fila por la derecha. */}
        {hint && <span className={`block text-pretty text-[13px] [overflow-wrap:anywhere] ${TONO[tone]}`}>{hint}</span>}
      </span>
    </span>
  );

  // Encendida, la fila gana el aire de dentro que pierde por fuera: el texto no se mueve
  // y el aro no pisa la letra.
  const luz = luce
    ? "-mx-2 rounded-lg bg-marca/10 px-3 ring-1 ring-marca/50"
    : "px-1";
  const comun = `transition-[background-color,box-shadow] duration-300 ${luz}`;

  if (stacked) {
    return (
      <div ref={yo} data-fila={id} className={`${comun} py-3`}>
        <div className="mb-2.5">{texto}</div>
        {control}
      </div>
    );
  }

  return (
    <div ref={yo} data-fila={id} className={`flex items-center justify-between gap-4 py-3 ${comun}`}>
      {texto}
      {control !== null && <span className="shrink-0">{control}</span>}
    </div>
  );
}

interface RowButtonProps {
  onClick: () => void;
  children: ReactNode;
  disabled?: boolean;
  /** Lo que sale al dejar el ratón encima, cuando el botón solo no se explica. */
  title?: string;
  /** Para el que hace algo que no se deshace solo. */
  danger?: boolean;
}

/**
 * El botón secundario de una fila: fondo suave y la letra con peso, a 13 px.
 *
 * Es un botón de verdad y no un enlace (los gustos: «la secundaria es un BOTÓN»). Hasta la
 * 0.5 era un contorno de 11,5 px, por debajo del mínimo de 12 que se lee sin acercarse.
 */
export function RowButton({
  onClick,
  children,
  disabled = false,
  danger = false,
  title,
}: RowButtonProps) {
  return (
    <button
      type="button"
      title={title}
      disabled={disabled}
      onClick={onClick}
      className={`whitespace-nowrap rounded-lg border px-3 py-1.5 text-[13px] font-semibold transition-colors disabled:opacity-40 ${
        danger
          ? "border-red-500/50 bg-red-500/15 text-[color:light-dark(#b91c1c,#fca5a5)] hover:bg-red-500/25"
          : "border-linea bg-realce text-titulo hover:bg-activo"
      }`}
    >
      {children}
    </button>
  );
}
