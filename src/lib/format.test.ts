/**
 * Los formateadores que comparten el overlay y el editor.
 *
 * Son cuatro cuentas cortas, pero salen escritas en pantalla cada pocos milisegundos
 * mientras se graba, y un redondeo mal puesto se ve enseguida: un contador que salta de
 * 0:59 a 1:60, o un archivo de 1.024 KB en vez de 1,0 MB.
 */
import { describe, expect, it } from "vitest";
import { clamp, formatBytes, formatDuration, formatTimecode } from "./format";

describe("la duracion, en minutos y segundos", () => {
  it("empieza en 0:00", () => {
    expect(formatDuration(0)).toBe("0:00");
  });

  it("pone el cero delante de los segundos de una cifra", () => {
    expect(formatDuration(5_000)).toBe("0:05");
  });

  it("pasa a minutos al llegar a sesenta segundos, no a 0:60", () => {
    expect(formatDuration(59_000)).toBe("0:59");
    expect(formatDuration(60_000)).toBe("1:00");
    expect(formatDuration(61_000)).toBe("1:01");
  });

  it("aguanta una grabacion larga sin inventarse las horas", () => {
    expect(formatDuration(3_600_000)).toBe("60:00");
  });

  it("un tiempo negativo se ensenna como cero, no con un menos delante", () => {
    expect(formatDuration(-500)).toBe("0:00");
  });
});

describe("el codigo de tiempo del editor, con centesimas", () => {
  it("lleva dos decimales siempre", () => {
    expect(formatTimecode(0)).toBe("0:00.00");
    expect(formatTimecode(1_500)).toBe("0:01.50");
  });

  it("las centesimas se truncan, no se redondean hacia arriba", () => {
    // 1.999 ms es el ultimo instante del segundo 1: ensennar 0:02.00 seria mentir sobre
    // en que fotograma esta el cursor.
    expect(formatTimecode(1_999)).toBe("0:01.99");
  });

  it("cuenta los minutos igual que la duracion", () => {
    expect(formatTimecode(65_120)).toBe("1:05.12");
  });
});

describe("el tamanno de un archivo", () => {
  it("por debajo de un kilo va en bytes tal cual", () => {
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(1_023)).toBe("1023 B");
  });

  it("de un kilo a un mega, en KB redondos", () => {
    expect(formatBytes(1_024)).toBe("1 KB");
    expect(formatBytes(512_000)).toBe("500 KB");
  });

  it("a partir de un mega, con un decimal", () => {
    expect(formatBytes(1_048_576)).toBe("1.0 MB");
    expect(formatBytes(3_500_000)).toBe("3.3 MB");
  });

  it("un instalador entero se lee de un vistazo", () => {
    // Los 2.325.944 bytes que mide hoy el instalador de winshotx.
    expect(formatBytes(2_325_944)).toBe("2.2 MB");
  });
});

describe("clamp, que es lo que impide salirse", () => {
  it("deja pasar lo que ya esta dentro", () => {
    expect(clamp(5, 0, 10)).toBe(5);
  });

  it("recorta por los dos lados", () => {
    expect(clamp(-3, 0, 10)).toBe(0);
    expect(clamp(99, 0, 10)).toBe(10);
  });

  it("con limites negativos tambien, que es el caso de los monitores de la izquierda", () => {
    expect(clamp(-500, -1200, 0)).toBe(-500);
    expect(clamp(-5000, -1200, 0)).toBe(-1200);
  });

  it("con los limites cruzados manda el MAXIMO, y no avisa de nada", () => {
    // Queda escrito porque no es lo que se espera al leer el nombre: `Math.min(max, ...)`
    // va el ultimo, asi que gana el max. En Rust el equivalente entra en panico y aqui
    // devuelve un numero raro en silencio.
    //
    // Hoy no muerde: los dos sitios que lo usan pasan `clamp(i, 0, ultimo)` con `ultimo`
    // nunca por debajo de cero. Si algun dia un limite viene de dos sitios distintos, esta
    // prueba dice lo que va a pasar.
    expect(clamp(5, 10, 3)).toBe(3);
  });
});
