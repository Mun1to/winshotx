/**
 * Lo dibujado sobre la captura, cuál está elegido, y cómo se deshace.
 *
 * Deshacer era quitar la última marca, y con eso bastaba mientras lo único que se podía
 * hacer era añadir. Desde que lo dibujado se mueve, se edita y se borra suelto, Ctrl+Z
 * tiene que devolver lo de ANTES del último cambio: la flecha a donde estaba, el texto como
 * decía, lo borrado a su sitio. Así que cada cambio guarda la lista entera de antes, que
 * son unas pocas marcas de unos pocos números: copiarla cuesta nada y no hay forma de que
 * deshacer y rehacer se desincronicen.
 *
 * Elegir no es un cambio: no se deshace, porque Ctrl+Z que solo quitara el halo de lo
 * elegido parecería que no ha hecho nada.
 */
import { conEscala, escalaDe, renumerarPasos, seEscala, type Anotacion } from "./anotaciones";

export interface Marcas {
  /** Lo dibujado, en el orden en que se pinta: lo último va encima. */
  lista: Anotacion[];
  /** Cómo estaba la lista antes de cada cambio, el más reciente al final. */
  antes: Anotacion[][];
  /** El sitio en `lista` de la marca elegida, o `null` si no hay ninguna. */
  elegida: number | null;
}

export type CambioDeMarcas =
  | { tipo: "anadir"; marcas: Anotacion[] }
  | { tipo: "cambiar"; indice: number; marca: Anotacion }
  | { tipo: "borrar"; indice: number }
  | { tipo: "borrarTodo" }
  | { tipo: "deshacer" }
  | { tipo: "elegir"; indice: number | null }
  /**
   * S24: pinchar un color con algo elegido se lo cambia a eso. Va aparte de `cambiar` para
   * que lo haga el reductor con la lista de ese momento: si un texto estaba abierto, pinchar
   * el color lo guarda primero, y con la marca copiada al pinchar se perdería lo escrito.
   */
  | { tipo: "colorear"; indice: number; color: string }
  /**
   * El tamaño de un texto o de un paso elegido, desde los botones de la barra (issues 18 y
   * 19). Va aparte de `cambiar` por lo mismo que el color: si el texto estaba abierto, se
   * aplica a la marca de ese momento.
   */
  | { tipo: "escalar"; indice: number; escala: number };

/**
 * Las marcas que llevan el color que se elige. El resaltado es siempre amarillo (es un
 * marcador), el mosaico tapa y el foco oscurece: a esas el color no les dice nada.
 */
export function seColorea(marca: Anotacion | undefined): boolean {
  return !!marca && !["highlight", "blur", "focus"].includes(marca.kind);
}

export const SIN_MARCAS: Marcas = { lista: [], antes: [], elegida: null };

/** Cuántos cambios se pueden deshacer. Más que esto no lo deshace nadie a mano. */
const RECUERDA = 200;

export function aplicarCambio(m: Marcas, c: CambioDeMarcas): Marcas {
  const con = (lista: Anotacion[], elegida: number | null): Marcas => ({
    lista,
    antes: [...m.antes, m.lista].slice(-RECUERDA),
    elegida,
  });
  switch (c.tipo) {
    case "anadir":
      return c.marcas.length > 0 ? con([...m.lista, ...c.marcas], m.elegida) : m;
    case "cambiar": {
      const vieja = m.lista[c.indice];
      // Un clic sin mover, o un texto que se abre y se cierra sin tocarlo, no es un cambio:
      // si contara, el siguiente Ctrl+Z no haría nada que se viera.
      if (!vieja || JSON.stringify(vieja) === JSON.stringify(c.marca)) return m;
      return con(
        m.lista.map((a, i) => (i === c.indice ? c.marca : a)),
        m.elegida,
      );
    }
    case "borrar":
      if (!m.lista[c.indice]) return m;
      return con(renumerarPasos(m.lista.filter((_, i) => i !== c.indice)), null);
    case "borrarTodo":
      return m.lista.length > 0 ? con([], null) : m;
    case "deshacer":
      // Lo elegido se suelta: después de deshacer, el mismo sitio de la lista puede ser
      // otra marca, y borrar con Supr se llevaría la que no es.
      if (m.antes.length === 0) return m;
      return { lista: m.antes[m.antes.length - 1], antes: m.antes.slice(0, -1), elegida: null };
    case "elegir": {
      const indice = c.indice !== null && m.lista[c.indice] ? c.indice : null;
      return indice === m.elegida ? m : { ...m, elegida: indice };
    }
    case "colorear": {
      const vieja = m.lista[c.indice];
      // Su mismo color no es un cambio: si contara, el siguiente Ctrl+Z no haría nada.
      if (!seColorea(vieja) || vieja.color === c.color) return m;
      return con(
        m.lista.map((a, i) => (i === c.indice ? { ...a, color: c.color } : a)),
        m.elegida,
      );
    }
    case "escalar": {
      const vieja = m.lista[c.indice];
      // Lo que no lleva tamaño no se toca, y su mismo tamaño no gasta un Ctrl+Z.
      if (!seEscala(vieja) || escalaDe(vieja) === c.escala) return m;
      return con(
        m.lista.map((a, i) => (i === c.indice ? conEscala(a, c.escala) : a)),
        m.elegida,
      );
    }
  }
}
