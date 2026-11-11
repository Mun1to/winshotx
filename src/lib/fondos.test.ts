/**
 * Los fondos de la vista previa tienen que ser los del archivo. Los números del degradado
 * son los mismos que comprueba `marco.rs` (`el_segundo_color_es_el_mismo_que_calcula_el_editor`):
 * si cambia la cuenta en un lado y no en el otro, se pone rojo uno de los dos.
 */
import { describe, expect, it } from "vitest";
import {
  cssDelFondo,
  hexDe,
  hsvDe,
  recordarEscritorio,
  rgbDeHex,
  rgbDeHsv,
  segundoColor,
  vistaDelVideo,
  type Rgb,
} from "./fondos";

describe("el degradado de un color", () => {
  it("sale con los mismos números que en Rust", () => {
    expect(segundoColor([88, 116, 245])).toEqual([134, 31, 242]);
    expect(segundoColor([34, 197, 152])).toEqual([28, 109, 162]);
    expect(segundoColor([255, 255, 255])).toEqual([209, 209, 209]);
    expect(segundoColor([0, 0, 0])).toEqual([0, 0, 0]);
    expect(segundoColor([220, 40, 40])).toEqual([183, 132, 30]);
    expect(segundoColor([18, 18, 20])).toEqual([16, 15, 16]);
  });
});

describe("el código del color", () => {
  it("se lee con o sin almohadilla, y en corto", () => {
    expect(rgbDeHex("#3e5ba9")).toEqual([62, 91, 169]);
    expect(rgbDeHex("3E5BA9")).toEqual([62, 91, 169]);
    expect(rgbDeHex("#35a")).toEqual([51, 85, 170]);
    expect(rgbDeHex(" #ffffff ")).toEqual([255, 255, 255]);
  });

  it("lo que no es un color no se lee", () => {
    expect(rgbDeHex("azul")).toBeNull();
    expect(rgbDeHex("#12345")).toBeNull();
    expect(rgbDeHex("")).toBeNull();
  });

  it("se escribe en minúsculas y con sus dos cifras por canal", () => {
    expect(hexDe([0, 10, 255])).toBe("#000aff");
  });
});

describe("la paleta", () => {
  it("ida y vuelta entre RGB y tono, saturación y brillo sin perder el color", () => {
    const colores: Rgb[] = [[62, 91, 169], [255, 0, 0], [0, 0, 0], [255, 255, 255], [12, 200, 99], [128, 128, 128]];
    for (const rgb of colores) {
      expect(rgbDeHsv(...hsvDe(rgb))).toEqual(rgb);
    }
  });

  it("el tono va de 0 a 360 y un gris no tiene saturación", () => {
    expect(hsvDe([255, 0, 0])[0]).toBe(0);
    expect(Math.round(hsvDe([0, 0, 255])[0])).toBe(240);
    expect(hsvDe([128, 128, 128])[1]).toBe(0);
  });
});

describe("cómo se pinta cada fondo", () => {
  it("sin uno propio, el de nombre de siempre", () => {
    expect(cssDelFondo("negro")).toBe("rgb(18 18 20)");
    expect(cssDelFondo("menta", null)).toContain("linear-gradient");
  });

  it("un color propio manda sobre el de nombre", () => {
    expect(cssDelFondo("negro", { tipo: "color", rgb: [1, 2, 3] })).toBe("rgb(1 2 3)");
  });

  it("el degradado propio va del color a su segundo color", () => {
    expect(cssDelFondo("blanco", { tipo: "degradado", rgb: [220, 40, 40] })).toBe(
      "linear-gradient(135deg, rgb(220 40 40), rgb(183 132 30))",
    );
  });

  it("una imagen se enseña por el protocolo de la app y recortada para llenar", () => {
    const css = cssDelFondo("blanco", { tipo: "imagen", ruta: "C:\\fondos\\imagen-1.jpg" });
    expect(css).toContain('url("asset://C:\\fondos\\imagen-1.jpg")');
    expect(css).toContain("cover");
  });

  it("un vídeo se enseña con su primer fotograma, que está al lado", () => {
    expect(vistaDelVideo("C:\\fondos\\video-9.mp4")).toBe("C:\\fondos\\video-9.jpg");
    expect(cssDelFondo("blanco", { tipo: "video", ruta: "C:\\fondos\\video-9.mp4" })).toContain("video-9.jpg");
  });

  it("el escritorio enseña lo que se trajo al elegirlo", () => {
    recordarEscritorio({ rgb: [0, 120, 215] });
    expect(cssDelFondo("blanco", { tipo: "escritorio" })).toBe("rgb(0 120 215)");
    recordarEscritorio({ imagen: "C:\\fondos\\escritorio-1.jpg" });
    expect(cssDelFondo("blanco", { tipo: "escritorio" })).toContain("escritorio-1.jpg");
    recordarEscritorio(null);
    expect(cssDelFondo("blanco", { tipo: "escritorio" })).toBe("rgb(18 18 20)");
  });
});
