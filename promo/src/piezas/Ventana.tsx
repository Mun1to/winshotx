import type { CSSProperties } from "react";
import { Img } from "remotion";
import { foto } from "../app";

/**
 * Una ventana de la app, quieta, sacada del banco de fotos (editor o ajustes), puesta en el
 * escenario con el radio y la sombra de una ventana de Windows 11. El marco de Windows no se
 * dibuja: sería inventarlo.
 */
export function Ventana({ img, x, y, ancho, alto, style }: { img: string; x: number; y: number; ancho: number; alto: number; style?: CSSProperties }) {
  return (
    <div
      style={{
        position: "absolute",
        left: x,
        top: y,
        width: ancho,
        height: alto,
        borderRadius: 10,
        overflow: "hidden",
        boxShadow: "0 30px 80px -20px rgba(0,0,0,0.85), 0 0 0 1px rgba(255,255,255,0.08)",
        ...style,
      }}
    >
      <Img src={foto(img)} style={{ width: ancho, height: alto, display: "block" }} />
    </div>
  );
}
