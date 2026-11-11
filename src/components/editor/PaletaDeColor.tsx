/**
 * Una paleta de color propia: el cuadro de saturación y brillo, la barra del tono y el
 * código del color para pegarlo de otro sitio.
 *
 * Propia y no el selector de color de Windows (`<input type="color">`): ese abre una
 * ventana del sistema con otra cara, que es justo lo que Munir no quiere en sus productos
 * («controles propios con la cara del producto»). Y el color cambia mientras se arrastra,
 * para verlo en la vista previa sin soltar.
 */
import { useEffect, useRef, useState, type KeyboardEvent, type PointerEvent } from "react";
import { useT } from "../../lib/i18n";
import { hexDe, hsvDe, rgbDeHex, rgbDeHsv, type Rgb } from "../../lib/fondos";

interface Props {
  rgb: Rgb;
  onCambiar: (rgb: Rgb) => void;
}

const limitar = (v: number) => Math.min(1, Math.max(0, v));

export function PaletaDeColor({ rgb, onCambiar }: Props) {
  const t = useT();
  // El tono va aparte del color: en un gris o en el negro el RGB no lo sabe, y sin esto la
  // barra del tono saltaría al rojo en cuanto el punto tocara un borde del cuadro.
  const [hsv, setHsv] = useState(() => hsvDe(rgb));
  const [texto, setTexto] = useState(() => hexDe(rgb));
  const cuadro = useRef<HTMLDivElement>(null);
  const barra = useRef<HTMLDivElement>(null);

  // Si el color cambia desde fuera, la paleta se pone en él sin perder su tono.
  useEffect(() => {
    setTexto(hexDe(rgb));
    setHsv((antes) => {
      if (hexDe(rgbDeHsv(...antes)) === hexDe(rgb)) return antes;
      const nuevo = hsvDe(rgb);
      return nuevo[1] === 0 || nuevo[2] === 0 ? [antes[0], nuevo[1], nuevo[2]] : nuevo;
    });
  }, [rgb]);

  const poner = (h: number, s: number, v: number) => {
    setHsv([h, s, v]);
    const nuevo = rgbDeHsv(h, s, v);
    setTexto(hexDe(nuevo));
    onCambiar(nuevo);
  };

  const [h, s, v] = hsv;

  const enElCuadro = (e: PointerEvent<HTMLDivElement>) => {
    const caja = cuadro.current?.getBoundingClientRect();
    if (!caja || caja.width === 0 || caja.height === 0) return;
    poner(h, limitar((e.clientX - caja.left) / caja.width), limitar(1 - (e.clientY - caja.top) / caja.height));
  };

  const enLaBarra = (e: PointerEvent<HTMLDivElement>) => {
    const caja = barra.current?.getBoundingClientRect();
    if (!caja || caja.width === 0) return;
    poner(limitar((e.clientX - caja.left) / caja.width) * 359.9, s, v);
  };

  /** Arrastrar: se captura el puntero para que siga aunque se salga del cuadro. */
  const arrastre = (mover: (e: PointerEvent<HTMLDivElement>) => void) => ({
    onPointerDown: (e: PointerEvent<HTMLDivElement>) => {
      e.currentTarget.setPointerCapture?.(e.pointerId);
      mover(e);
    },
    onPointerMove: (e: PointerEvent<HTMLDivElement>) => {
      if (e.currentTarget.hasPointerCapture?.(e.pointerId)) mover(e);
    },
  });

  const teclasDelCuadro = (e: KeyboardEvent) => {
    const paso = e.shiftKey ? 0.1 : 0.02;
    const cambio: Record<string, [number, number]> = {
      ArrowLeft: [-paso, 0],
      ArrowRight: [paso, 0],
      ArrowUp: [0, paso],
      ArrowDown: [0, -paso],
    };
    const [ds, dv] = cambio[e.key] ?? [0, 0];
    if (ds === 0 && dv === 0) return;
    e.preventDefault();
    poner(h, limitar(s + ds), limitar(v + dv));
  };

  const teclasDeLaBarra = (e: KeyboardEvent) => {
    const paso = e.shiftKey ? 30 : 5;
    const d = e.key === "ArrowLeft" || e.key === "ArrowDown" ? -paso : e.key === "ArrowRight" || e.key === "ArrowUp" ? paso : 0;
    if (d === 0) return;
    e.preventDefault();
    poner((h + d + 360) % 360, s, v);
  };

  const leerTexto = () => {
    const leido = rgbDeHex(texto);
    if (!leido) {
      setTexto(hexDe(rgbDeHsv(h, s, v)));
      return;
    }
    const [nh, ns, nv] = hsvDe(leido);
    setHsv([ns === 0 ? h : nh, ns, nv]);
    setTexto(hexDe(leido));
    onCambiar(leido);
  };

  return (
    <div className="space-y-2.5">
      <div
        ref={cuadro}
        role="slider"
        tabIndex={0}
        aria-label={t("Saturación y brillo")}
        aria-valuetext={hexDe(rgbDeHsv(h, s, v))}
        onKeyDown={teclasDelCuadro}
        {...arrastre(enElCuadro)}
        className="relative h-[120px] w-full cursor-crosshair touch-none rounded-md outline-none focus-visible:ring-2 focus-visible:ring-blue-500"
        style={{
          background: `linear-gradient(to top, rgb(0 0 0), transparent), linear-gradient(to right, rgb(255 255 255), hsl(${h} 100% 50%))`,
        }}
      >
        <span
          className="pointer-events-none absolute size-4 -translate-x-1/2 -translate-y-1/2 rounded-full border-2 border-white shadow-[0_0_0_1px_rgb(0_0_0/0.4)]"
          style={{ left: `${s * 100}%`, top: `${(1 - v) * 100}%`, background: hexDe(rgbDeHsv(h, s, v)) }}
        />
      </div>
      <div
        ref={barra}
        role="slider"
        tabIndex={0}
        aria-label={t("Tono")}
        aria-valuemin={0}
        aria-valuemax={360}
        aria-valuenow={Math.round(h)}
        onKeyDown={teclasDeLaBarra}
        {...arrastre(enLaBarra)}
        className="relative h-3.5 w-full cursor-pointer touch-none rounded-full outline-none focus-visible:ring-2 focus-visible:ring-blue-500"
        style={{
          background:
            "linear-gradient(to right, hsl(0 100% 50%), hsl(60 100% 50%), hsl(120 100% 50%), hsl(180 100% 50%), hsl(240 100% 50%), hsl(300 100% 50%), hsl(360 100% 50%))",
        }}
      >
        <span
          className="pointer-events-none absolute top-1/2 size-4 -translate-x-1/2 -translate-y-1/2 rounded-full border-2 border-white shadow-[0_0_0_1px_rgb(0_0_0/0.4)]"
          style={{ left: `${(h / 360) * 100}%`, background: `hsl(${h} 100% 50%)` }}
        />
      </div>
      <label className="flex items-center gap-2">
        <span className="text-[13px] font-medium text-texto">{t("Código del color")}</span>
        <input
          type="text"
          value={texto}
          spellCheck={false}
          aria-label={t("Código del color")}
          onChange={(e) => setTexto(e.target.value)}
          onBlur={leerTexto}
          onKeyDown={(e) => {
            if (e.key === "Enter") leerTexto();
          }}
          className="ml-auto w-[96px] rounded-lg border border-linea-fuerte bg-hueco px-2.5 py-1 text-[13px] tabular-nums text-titulo outline-none focus:border-blue-500/60"
        />
      </label>
    </div>
  );
}
