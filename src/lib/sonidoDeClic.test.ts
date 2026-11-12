/**
 * El sonido de clic en la vista previa (S10): suenan los mismos clics que en el MP4, y lo que
 * falta para cada uno se cuenta en el tiempo de verdad de la reproducción.
 */
import { describe, expect, it } from "vitest";
import { clicsQueSuenan, gananciaDelClic, nombreDelClic, relojDeSalida } from "./sonidoDeClic";
import type { Pieza } from "./tramo";

// Lo mismo que la prueba de Rust `un_clic_cortado_o_acelerado_no_suena_y_los_demas_se_corren`:
// se queda de 0 a 1 s, se corta de 1 a 3 s, de 3 a 5 s va al doble y de 5 a 6 s a su velocidad.
const PIEZAS: Pieza[] = [
  { desdeMs: 0, hastaMs: 1_000, velocidad: 1, cortada: false },
  { desdeMs: 1_000, hastaMs: 3_000, velocidad: 1, cortada: true },
  { desdeMs: 3_000, hastaMs: 5_000, velocidad: 2, cortada: false },
  { desdeMs: 5_000, hastaMs: 6_000, velocidad: 1, cortada: false },
];

describe("el sonido de clic en la vista previa", () => {
  it("suenan los clics de lo que se queda a su velocidad, como en el MP4", () => {
    expect(clicsQueSuenan([5_200, 500, 2_000, 4_000], PIEZAS, 0, 6_000)).toEqual([500, 5_200]);
    // Fuera de las asas tampoco suena, y sin piezas cuenta la grabación entera.
    expect(clicsQueSuenan([500, 5_200], PIEZAS, 600, 6_000)).toEqual([5_200]);
    expect(clicsQueSuenan([100, 900], [], 0, 1_000)).toEqual([100, 900]);
  });

  it("el reloj de salida salta lo cortado y cuenta lo acelerado a su velocidad", () => {
    const salida = relojDeSalida(PIEZAS, 0, 6_000);
    expect(salida(500)).toBe(500);
    expect(salida(2_000)).toBe(1_000);
    expect(salida(4_000)).toBe(1_500);
    // El clic de 5,2 s cae a los 2,2 s del vídeo: donde lo pone `clic::instantes`.
    expect(salida(5_200)).toBe(2_200);
  });

  it("pesa lo mismo que en Rust, y el nombre del propio es el de su archivo", () => {
    expect(gananciaDelClic(0.5)).toBeCloseTo(0.3);
    expect(gananciaDelClic(4)).toBeCloseTo(0.6);
    expect(nombreDelClic("C:\\Users\\a\\AppData\\Roaming\\winshotx\\sonidos\\clic-0123456789abcdef-mi clic.wav")).toBe("mi clic");
  });
});
