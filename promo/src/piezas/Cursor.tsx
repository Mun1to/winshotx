/**
 * El puntero de Windows, dibujado: una foto sin ventana no lo lleva. La flecha blanca con
 * borde negro de siempre y, mientras se elige la zona, la cruz fina que pone el overlay
 * (`cursor: crosshair` en `SelectionCanvas.tsx`).
 */
export function Cursor({ x, y, forma }: { x: number; y: number; forma: "flecha" | "cruz" }) {
  if (forma === "cruz") {
    return (
      <svg width={34} height={34} viewBox="-17 -17 34 34" style={{ position: "absolute", left: x - 17, top: y - 17, overflow: "visible" }}>
        <path d="M0 -14V14M-14 0H14" stroke="#000" strokeWidth={4} />
        <path d="M0 -13V13M-13 0H13" stroke="#fff" strokeWidth={1.6} />
      </svg>
    );
  }
  return (
    <svg width={30} height={44} viewBox="0 0 12 19" style={{ position: "absolute", left: x - 1, top: y - 1 }}>
      <path d="M1 1v15.2l3.6-3.4 2.4 5.4 2.3-1-2.3-5.3H12z" fill="#fff" stroke="#000" strokeWidth={0.9} strokeLinejoin="round" />
    </svg>
  );
}
