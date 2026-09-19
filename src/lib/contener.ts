/**
 * Dónde cae de verdad una imagen dentro de su hueco.
 *
 * La vista previa del editor se ajusta con `object-contain`: se hace todo lo grande que
 * puede sin deformarse, y deja franjas a los lados o arriba y abajo. Encima van las capas
 * de dibujar y de recortar, cuyas coordenadas se guardan **de 0 a 1 sobre la captura**.
 *
 * Si esas capas se estiran al hueco entero, sus coordenadas cuentan las franjas como si
 * fueran parte de la captura: lo que se dibuja en el borde de la imagen se guarda un poco
 * más allá, y al exportar sale desplazado. Con una captura vertical en una ventana ancha,
 * el desplazamiento es de media pantalla.
 *
 * Se calcula a mano y no con `aspect-ratio` en CSS: dentro de una caja que además tiene
 * contenido propio, esa propiedad se resuelve de maneras distintas según el tipo de
 * contenedor, y aquí hace falta un número exacto y no una negociación.
 */
export interface Caja {
  width: number;
  height: number;
}

/**
 * Lo que mide la imagen dentro del hueco, sin deformarse y sin salirse.
 *
 * Nunca devuelve un lado de cero: un hueco todavía sin medir (la primera pasada, antes de
 * que el navegador haya colocado nada) dejaría las capas en un punto y cualquier arrastre
 * dividiría por cero.
 */
export function contener(
  huecoAncho: number,
  huecoAlto: number,
  ancho: number,
  alto: number,
): Caja {
  if (huecoAncho <= 0 || huecoAlto <= 0 || ancho <= 0 || alto <= 0) {
    return { width: 0, height: 0 };
  }
  const escala = Math.min(huecoAncho / ancho, huecoAlto / alto);
  return {
    width: Math.max(1, Math.round(ancho * escala)),
    height: Math.max(1, Math.round(alto * escala)),
  };
}
