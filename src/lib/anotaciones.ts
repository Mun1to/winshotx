/**
 * Las marcas que se pueden dibujar sobre una captura antes de exportarla.
 *
 * Las seis de siempre (flecha, rectángulo, texto, resaltado, paso y mosaico) y las cinco
 * que entraron el 11 de noviembre de 2026 con el plan de CleanShot X: elipse, línea, lápiz,
 * foco y flecha curva. **Y aquí se para**: el editor de imagen no es el producto. Lo que
 * hace falta para señalar algo, para tapar un dato o para explicar un orden está aquí; lo
 * demás es otro programa.
 *
 * Quien pinta de verdad es Rust (`src-tauri/src/encode/anotacion.rs`). Lo que hay aquí es
 * la geometría que comparten los dos lados: la vista previa la dibuja en SVG con estas
 * mismas cuentas, y las pruebas de los dos lados leen las mismas marcas de
 * `marcas-de-prueba.json`.
 */
export type Herramienta =
  | "arrow"
  | "curve"
  | "line"
  | "box"
  | "ellipse"
  | "pen"
  | "text"
  | "highlight"
  | "step"
  | "focus"
  | "blur";

/** Un punto de 0 a 1 sobre el ancho y el alto de la imagen. */
export type Punto = [number, number];

/**
 * Una marca, con sus dos esquinas **de 0 a 1** sobre el ancho y el alto de la imagen.
 *
 * En tanto por uno y no en píxeles porque quien dibuja lo hace sobre una vista previa que
 * casi nunca mide lo que va a medir el archivo. Así el mismo dibujo vale exportando al
 * 50 % o al doble, sin recalcular nada.
 */
export interface Anotacion {
  kind: Herramienta;
  x1: number;
  y1: number;
  x2: number;
  y2: number;
  /** En `#rrggbb`. El mosaico y el foco lo ignoran. */
  color: string;
  /** Lo que dice, si es un texto. Y en un paso, su número. */
  text: string;
  /**
   * Los puntos de más de dos marcas. En el lápiz, el trazo entero; en la flecha curva,
   * uno solo: el punto de control, de donde «tira» la curva. Las demás no lo llevan.
   */
  puntos?: Punto[];
}

/** Los colores de las marcas. Cinco, y el primero es el de señalar. */
export const COLORES = ["#ef4444", "#0a9bff", "#22c55e", "#fbbf24", "#111827"];

/** El color con el que sale el resaltado, que no se elige: un marcador es amarillo. */
export const COLOR_RESALTADO = "#fbbf24";

/**
 * Cuánto oscurece el foco lo que queda fuera de su zona. Es `FOCO_OSCURECE` de Rust, y
 * `marcas-de-prueba.json` lo lleva escrito para que las dos pruebas se pongan rojas si
 * alguien cambia uno sin el otro.
 */
export const FOCO_OSCURECE = 0.55;

/**
 * Un punto de la flecha curva, con `t` de 0 (el origen) a 1 (la punta).
 *
 * Bézier cuadrática, igual que `encode::anotacion::punto_de_curva` y que la `Q` del SVG:
 * sale de `p0`, llega a `p2`, y `c` tira de ella sin que la curva llegue a tocarlo.
 */
export function puntoDeCurva(p0: Punto, c: Punto, p2: Punto, t: number): Punto {
  const u = 1 - t;
  return [
    u * u * p0[0] + 2 * u * t * c[0] + t * t * p2[0],
    u * u * p0[1] + 2 * u * t * c[1] + t * t * p2[1],
  ];
}

/**
 * El punto de control de una flecha curva, sacado del camino que hizo el ratón.
 *
 * Nadie quiere colocar un punto de control a mano: se dibuja la curva y ya. Se busca el
 * punto del camino que queda a mitad de recorrido, y se pone el control donde hace falta
 * para que la curva pase JUSTO por ahí a la mitad (con t = 0,5 la cuadrática pasa por el
 * punto medio entre el control y la media de los extremos, así que se despeja).
 *
 * Si el camino es casi recto, el control cae en medio y la flecha sale recta: un pulso
 * que tiembla no debería doblar nada.
 */
export function controlDeCurva(camino: Punto[]): Punto {
  const p0 = camino[0];
  const p2 = camino[camino.length - 1];
  const recto: Punto = [(p0[0] + p2[0]) / 2, (p0[1] + p2[1]) / 2];
  if (camino.length < 3) return recto;
  // El recorrido acumulado, para dar con el punto que queda a la mitad.
  const tramos = camino.slice(1).map((p, i) => Math.hypot(p[0] - camino[i][0], p[1] - camino[i][1]));
  const total = tramos.reduce((a, b) => a + b, 0);
  if (total === 0) return recto;
  let andado = 0;
  let medio: Punto = recto;
  for (let i = 0; i < tramos.length; i++) {
    if (andado + tramos[i] >= total / 2) {
      const f = tramos[i] === 0 ? 0 : (total / 2 - andado) / tramos[i];
      medio = [
        camino[i][0] + (camino[i + 1][0] - camino[i][0]) * f,
        camino[i][1] + (camino[i + 1][1] - camino[i][1]) * f,
      ];
      break;
    }
    andado += tramos[i];
  }
  // Cuánto se separa la mitad del camino de la recta, contra lo largo que es la flecha.
  const cuerda = Math.hypot(p2[0] - p0[0], p2[1] - p0[1]);
  const separacion = Math.hypot(medio[0] - recto[0], medio[1] - recto[1]);
  if (cuerda === 0 || separacion / cuerda < 0.04) return recto;
  return [2 * medio[0] - recto[0], 2 * medio[1] - recto[1]];
}

/**
 * Aclara un trazo a mano: quita los puntos que no cambian el dibujo.
 *
 * El ratón manda un punto cada pocos milisegundos, y un garabato de dos segundos son
 * cientos de puntos casi en línea. Se quedan los que doblan el trazo más de `tolerancia`
 * (Ramer-Douglas-Peucker), que con 0,001 es una fracción de píxel en la vista previa: el
 * dibujo sale igual y la petición de exportar pesa una décima parte.
 */
export function simplificarTrazo(puntos: Punto[], tolerancia = 0.001): Punto[] {
  if (puntos.length < 3) return puntos;
  const dentro = (desde: number, hasta: number, quedan: boolean[]) => {
    const [a, b] = [puntos[desde], puntos[hasta]];
    const largo = Math.hypot(b[0] - a[0], b[1] - a[1]);
    let peor = -1;
    let lejos = 0;
    for (let i = desde + 1; i < hasta; i++) {
      const p = puntos[i];
      const d =
        largo === 0
          ? Math.hypot(p[0] - a[0], p[1] - a[1])
          : Math.abs((b[0] - a[0]) * (a[1] - p[1]) - (a[0] - p[0]) * (b[1] - a[1])) / largo;
      if (d > lejos) {
        lejos = d;
        peor = i;
      }
    }
    if (peor !== -1 && lejos > tolerancia) {
      quedan[peor] = true;
      dentro(desde, peor, quedan);
      dentro(peor, hasta, quedan);
    }
  };
  const quedan = puntos.map((_, i) => i === 0 || i === puntos.length - 1);
  dentro(0, puntos.length - 1, quedan);
  return puntos.filter((_, i) => quedan[i]);
}
