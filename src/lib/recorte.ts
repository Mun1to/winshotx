/**
 * Quedarse con un trozo de la captura antes de exportarla.
 *
 * El editor ya recortaba el TIEMPO con los marcadores A y B; esto es la otra mitad. En una
 * foto siempre se puede volver a capturar, pero una grabación de tres minutos con el
 * encuadre torcido no se repite: o se recorta, o se tira.
 *
 * **Las coordenadas van de 0 a 1**, como las de las anotaciones y por lo mismo: quien
 * arrastra el marco lo hace sobre una vista previa que casi nunca mide lo que va a medir
 * el archivo.
 */
export interface Recorte {
  x1: number;
  y1: number;
  x2: number;
  y2: number;
}

/** Las dos esquinas ordenadas y metidas dentro de [0, 1]. */
export function ordenar(r: Recorte): Recorte {
  const dentro = (v: number) => Math.min(1, Math.max(0, v));
  return {
    x1: dentro(Math.min(r.x1, r.x2)),
    y1: dentro(Math.min(r.y1, r.y2)),
    x2: dentro(Math.max(r.x1, r.x2)),
    y2: dentro(Math.max(r.y1, r.y2)),
  };
}

/**
 * Cuánto mide el trozo en píxeles de una captura de ese tamaño.
 *
 * Nunca menos de dos píxeles por lado: H.264 no acepta un lado impar y tampoco tiene
 * sentido exportar un vídeo de un píxel de ancho.
 */
export function medida(r: Recorte, ancho: number, alto: number): { width: number; height: number } {
  const o = ordenar(r);
  return {
    width: Math.max(2, Math.round((o.x2 - o.x1) * ancho)),
    height: Math.max(2, Math.round((o.y2 - o.y1) * alto)),
  };
}

/**
 * Si el marco de verdad recorta algo.
 *
 * Un arrastre de dos píxeles sin querer, o uno que abarca la captura entera, no es un
 * recorte: es trabajo para no cambiar nada, y además dejaría el editor diciendo que hay un
 * recorte puesto cuando no lo hay.
 */
export function recortaAlgo(r: Recorte): boolean {
  const o = ordenar(r);
  return o.x2 - o.x1 > 0.01 && o.y2 - o.y1 > 0.01 && (o.x2 - o.x1 < 0.995 || o.y2 - o.y1 < 0.995);
}
