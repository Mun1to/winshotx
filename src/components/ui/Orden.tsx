import { Fragment } from "react";
import { partirOrden } from "../../lib/i18n";

/**
 * Una orden de terminal dentro de un aviso («sudo dnf install tesseract tesseract-langpack-spa»).
 *
 * En su propio renglón, para que empiece siempre por el principio. Y palabra a palabra: suelta,
 * la orden se cortaba por un guion a mitad del nombre del paquete («tesseract-langpack-» y
 * debajo «spa»). Así solo se parte por los espacios, y al seleccionarla se copia entera.
 */
export function Orden({ texto, className = "" }: { texto: string; className?: string }) {
  return (
    <code className={`block font-mono ${className}`}>
      {texto.split(" ").map((palabra, i) => (
        <Fragment key={i}>
          {i > 0 && " "}
          <span className="whitespace-nowrap">{palabra}</span>
        </Fragment>
      ))}
    </code>
  );
}

/**
 * El texto de un aviso ya traducido. Si acaba en una orden de terminal («Para leer el texto
 * falta Tesseract. Se instala con: sudo apt install tesseract-ocr», en Linux), la orden va
 * debajo de la frase, en su renglón.
 */
export function TextoDeAviso({ texto }: { texto: string }) {
  const partes = partirOrden(texto);
  if (!partes) return <>{texto}</>;
  return (
    <>
      {partes.frase}
      <Orden texto={partes.orden} className="mt-0.5" />
    </>
  );
}
