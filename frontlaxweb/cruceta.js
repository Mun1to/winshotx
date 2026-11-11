/*
 * La cruceta de la portada: dos líneas que cruzan la cabecera por donde va el ratón, con
 * sus coordenadas, un recuadro que se engancha a lo que se puede pulsar (con su tamaño
 * debajo, como winshotx al capturar una ventana) y una onda al hacer clic.
 *
 * Es el componente «Crosshair» de React Bits, de David Haz, portado a JavaScript sin React
 * para winshotx.com. Original:
 * https://github.com/DavidHDev/react-bits/blob/main/src/content/Animations/Crosshair/Crosshair.jsx
 *
 * ESTE ARCHIVO NO ESTÁ BAJO LA LICENCIA MIT DEL RESTO DEL REPOSITORIO. Lleva la licencia
 * del original, que permite usarlo como parte de una web y prohíbe venderlo o
 * redistribuirlo suelto, también portado. Su aviso, tal cual:
 *
 * ---------------------------------------------------------------------------------------
 * MIT + Commons Clause License Condition v1.0
 *
 * Copyright (c) 2026 David Haz
 *
 * Permission is hereby granted, free of charge, to any person obtaining a copy
 * of this software and associated documentation files (the "Software"), to deal
 * in the Software without restriction, including without limitation the rights
 * to use, copy, modify, merge, publish, and distribute the Software **as part of an
 * application, website, or product**, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included in all
 * copies or substantial portions of the Software.
 *
 * Commons Clause Restriction
 *
 * You may use this Software, including for any commercial purpose, **so long as you do
 * not sell, sublicense, or redistribute the components themselves-whether alone, in a
 * bundle, or as a ported version.**
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
 * AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
 * OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
 * SOFTWARE.
 * ---------------------------------------------------------------------------------------
 *
 * Lo que cambia frente al original, y por qué:
 * - Está hecho con piezas de la página que solo se mueven con `transform` y `opacity`, no con
 *   un lienzo. Un lienzo del tamaño de la ventana obliga al navegador a copiarlo entero en
 *   cada fotograma: medido con la CPU a 4x, eso se llevaba un tercio del tiempo y la página
 *   bajaba de 60 fotogramas. Así lo mueve la tarjeta gráfica y la página ni se entera.
 * - Las líneas van DETRÁS del texto y de los botones: un adorno no tapa el contenido. Solo
 *   las etiquetas, las esquinas del enganche y la onda van encima, porque son pequeñas y
 *   salen mientras apuntas. Y todo dentro de la cabecera, que es lo que la recorta.
 * - Nunca esconde el cursor del sistema, y deja un hueco alrededor de la punta.
 * - Solo con ratón. En una pantalla táctil no se crea, y un dedo no la mueve.
 * - La política de movimiento de la casa: seguir al ratón es movimiento que dirige quien
 *   mira, así que el dial `--motion-gain` no lo encoge. Con `?motion=off` (dial a 0) la
 *   cruceta va pegada al ratón sin suavizado, aparece y se va sin fundido y no hace onda:
 *   no se mueve nada que no muevas tú.
 * - Las etiquetas, con la letra de la página y a 12 px como mínimo, en el azul de los
 *   botones (el de la marca no da contraste con letra blanca). Los colores salen del CSS,
 *   así que siguen solos al tema del sistema.
 * - Las coordenadas son las de la ventana, como las de una captura.
 * - Sin el efecto «glitch» ni la opción de esconder el cursor, que aquí no se usan.
 */
(() => {
  const zona = document.querySelector("[data-cruceta]");
  if (!zona) return;

  // Solo donde hay un ratón de verdad. Un portátil con pantalla táctil también tiene ratón,
  // por eso `any-`: la que decide luego es cada evento, que dice si viene de un dedo.
  if (!matchMedia("(any-hover: hover) and (any-pointer: fine)").matches) return;

  const OPCIONES = {
    opacidad: 0.75,
    hueco: 12, // lo que queda libre alrededor de la punta del cursor
    suavizado: 0.3,
    coordenadas: true,
    enganche: true,
    onda: true,
    objetivos: "a, button, code, .dato, .pilar",
  };
  // El fundido hacia los bordes y el grosor se eligen en el CSS, en `.cruceta`: son dibujo,
  // no movimiento. Las líneas van continuas, como la guía de la app al capturar.

  const raiz = document.documentElement;
  const directo = parseFloat(getComputedStyle(raiz).getPropertyValue("--motion-gain")) === 0;
  const barra = document.querySelector("nav");

  const pieza = (clase, padre) => {
    const el = document.createElement("div");
    el.className = clase;
    padre.appendChild(el);
    return el;
  };
  // Dos capas: las líneas por DETRÁS del contenido, y las etiquetas, las esquinas del
  // enganche y la onda por ENCIMA, que son pequeñas, salen solo mientras apuntas (como la
  // medida de la app al capturar) y por detrás del texto no se leían.
  const nuevaCapa = (clase) => {
    const c = document.createElement("div");
    c.className = clase;
    c.setAttribute("aria-hidden", "true");
    if (directo) c.dataset.directo = "1";
    zona.prepend(c);
    return c;
  };
  const capa = nuevaCapa("cruceta");
  const encima = nuevaCapa("cruceta cruceta-arriba");
  const lineas = pieza("cr-lineas", capa);
  const hIzq = pieza("cr-h cr-izq", lineas);
  const hDer = pieza("cr-h cr-der", lineas);
  const vArr = pieza("cr-v cr-arr", lineas);
  const vAba = pieza("cr-v cr-aba", lineas);
  const etqX = pieza("cr-etq", encima);
  const etqY = pieza("cr-etq", encima);
  const enganche = pieza("cr-enganche", encima);
  const esquinas = ["cr-esq cr-ai", "cr-esq cr-ad", "cr-esq cr-bd", "cr-esq cr-bi"].map((c) => pieza(c, enganche));
  const etqTam = pieza("cr-etq", enganche);

  const limitar = (v, min, max) => Math.min(max, Math.max(min, v));
  const estado = {
    x: 0, y: 0, tx: 0, ty: 0, dentro: false, visto: false, presencia: 0,
    objetivo: null,
    recuadro: { x: 0, y: 0, w: 0, h: 0, alfa: 0 },
  };
  let raf = 0;
  let ultimo = 0;

  // Escribir solo lo que cambia: poner el mismo texto o el mismo estilo otra vez en cada
  // fotograma también cuesta.
  const puesto = new WeakMap();
  const poner = (el, prop, valor) => {
    let p = puesto.get(el);
    if (!p) puesto.set(el, (p = {}));
    if (p[prop] === valor) return;
    p[prop] = valor;
    if (prop === "texto") el.textContent = valor;
    else el.style[prop] = valor;
  };
  // Las etiquetas tienen ancho fijo (en el CSS): así no hay que medirlas para que no se
  // salgan por un borde, y cambiar el número no recoloca nada.
  const ANCHO = 48, ANCHO_TAM = 92;

  // Todo lo que se mide se mide al principio del fotograma, antes de escribir nada: leer una
  // medida después de cambiar un estilo obliga a la página a recolocarse en ese momento.
  // Los números se cambian como mucho cada 80 ms, y siempre en el último fotograma: nadie lee
  // sesenta números por segundo, y cambiar un texto obliga a repintarlo (las líneas no).
  let textoA = 0;
  const pintar = (caja, arriba, cajaObjetivo, ahora, ultimoCuadro) => {
    const tocaTexto = ultimoCuadro || ahora - textoA >= 80;
    if (tocaTexto) textoA = ahora;
    const W = caja.width, H = caja.height;
    const x = estado.x - caja.left; // en coordenadas de la cabecera
    const y = estado.y - caja.top;
    const r = estado.recuadro;
    const g = OPCIONES.hueco;

    poner(lineas, "opacity", String(estado.presencia * OPCIONES.opacidad * (1 - 0.6 * r.alfa)));
    if (estado.presencia > 0.001) {
      const fy = Math.round(y), fx = Math.round(x);
      poner(hIzq, "transform", `translate3d(0, ${fy}px, 0) scaleX(${Math.max(0, x - g) / W})`);
      poner(hDer, "transform", `translate3d(0, ${fy}px, 0) scaleX(${Math.max(0, W - x - g) / W})`);
      poner(vArr, "transform", `translate3d(${fx}px, 0, 0) scaleY(${Math.max(0, y - g) / H})`);
      poner(vAba, "transform", `translate3d(${fx}px, 0, 0) scaleY(${Math.max(0, H - y - g) / H})`);

      // Las etiquetas van opacas (sin la opacidad de las líneas): transparentes se mezclaban
      // con el texto de debajo.
      const verCoordenadas = OPCIONES.coordenadas ? estado.presencia * (1 - r.alfa) : 0;
      poner(etqX, "opacity", String(verCoordenadas));
      poner(etqY, "opacity", String(verCoordenadas));
      if (verCoordenadas > 0.001) {
        const tx = String(Math.round(estado.x)), ty = String(Math.round(estado.y));
        if (tocaTexto) {
          poner(etqX, "texto", tx);
          poner(etqY, "texto", ty);
        }
        const ax = ANCHO;
        poner(etqX, "transform", `translate3d(${Math.round(limitar(x - ax / 2, 4, W - ax - 4))}px, ${Math.round(arriba)}px, 0)`);
        poner(etqY, "transform", `translate3d(8px, ${Math.round(limitar(y - 10, arriba, H - 24))}px, 0)`);
      }
    }

    poner(enganche, "opacity", String(r.alfa));
    if (r.alfa > 0.001) {
      const ex = r.x - caja.left, ey = r.y - caja.top;
      const brazo = Math.min(14, r.w / 3, r.h / 3) / 14;
      const lado = 14 * brazo;
      const sitios = [[ex, ey], [ex + r.w - lado, ey], [ex + r.w - lado, ey + r.h - lado], [ex, ey + r.h - lado]];
      esquinas.forEach((el, i) => {
        const [sx, sy] = sitios[i];
        poner(el, "transform", `translate3d(${sx.toFixed(1)}px, ${sy.toFixed(1)}px, 0) scale(${brazo.toFixed(3)})`);
      });
      if (OPCIONES.coordenadas && cajaObjetivo) {
        const b = cajaObjetivo;
        const t = `${Math.round(b.width)} × ${Math.round(b.height)}`;
        if (tocaTexto) poner(etqTam, "texto", t);
        // Encima de la esquina de arriba a la izquierda, que es donde pone la app la medida al
        // capturar (y donde suele haber hueco); si no cabe, debajo.
        const ty = ey - 26 >= arriba ? ey - 26 : ey + r.h + 6;
        poner(etqTam, "transform", `translate3d(${Math.round(limitar(ex, 4, W - ANCHO_TAM - 4))}px, ${Math.round(Math.min(ty, H - 24))}px, 0)`);
      }
    }
  };

  const paso = (ahora) => {
    raf = 0;
    const dt = Math.min(0.05, Math.max(0.001, (ahora - ultimo) / 1000));
    ultimo = ahora;
    const caja = zona.getBoundingClientRect();
    // La etiqueta de la x va arriba de lo que se ve de la cabecera: la barra de arriba tapa un trozo.
    const arriba = Math.max(0, barra.getBoundingClientRect().bottom - caja.top) + 8;
    const cajaObjetivo = estado.objetivo ? estado.objetivo.getBoundingClientRect() : null;

    const seguir = directo ? 1 : 1 - Math.exp(-dt / (0.008 + OPCIONES.suavizado * 0.16));
    estado.x += (estado.tx - estado.x) * seguir;
    estado.y += (estado.ty - estado.y) * seguir;
    const quiero = estado.dentro ? 1 : 0;
    estado.presencia += (quiero - estado.presencia) * (directo ? 1 : 1 - Math.exp(-dt / (quiero ? 0.12 : 0.25)));
    if (Math.abs(quiero - estado.presencia) < 0.002) estado.presencia = quiero;

    const r = estado.recuadro;
    const enganchado = OPCIONES.enganche && estado.dentro ? estado.objetivo : null;
    let meta;
    if (enganchado) {
      const b = cajaObjetivo;
      meta = { x: b.left - 6, y: b.top - 6, w: b.width + 12, h: b.height + 12 };
    } else {
      meta = { x: estado.x - 10, y: estado.y - 10, w: 20, h: 20 };
    }
    const salto = directo ? 1 : 1 - Math.exp(-dt / 0.085);
    r.x += (meta.x - r.x) * salto;
    r.y += (meta.y - r.y) * salto;
    r.w += (meta.w - r.w) * salto;
    r.h += (meta.h - r.h) * salto;
    const visible = enganchado ? 1 : 0;
    r.alfa += (visible - r.alfa) * (directo ? 1 : 1 - Math.exp(-dt / (visible ? 0.08 : 0.14)));
    if (Math.abs(visible - r.alfa) < 0.002) r.alfa = visible;

    const moviendose =
      Math.abs(estado.tx - estado.x) > 0.05 ||
      Math.abs(estado.ty - estado.y) > 0.05 ||
      estado.presencia !== quiero ||
      r.alfa !== visible ||
      (enganchado && Math.abs(meta.w - r.w) + Math.abs(meta.x - r.x) + Math.abs(meta.y - r.y) > 0.1);
    pintar(caja, arriba, cajaObjetivo, ahora, !moviendose);
    if (moviendose) raf = requestAnimationFrame(paso);
  };

  const despertar = () => {
    if (raf) return;
    ultimo = performance.now();
    raf = requestAnimationFrame(paso);
  };

  const elegir = (el) => {
    if (!OPCIONES.enganche || !(el instanceof Element)) return null;
    const hallado = el.closest(OPCIONES.objetivos);
    return hallado && zona.contains(hallado) ? hallado : null;
  };

  const entrar = (e) => {
    estado.tx = e.clientX;
    estado.ty = e.clientY;
    if (!estado.visto) {
      estado.visto = true;
      estado.x = estado.tx;
      estado.y = estado.ty;
      Object.assign(estado.recuadro, { x: estado.x - 10, y: estado.y - 10, w: 20, h: 20 });
    }
    estado.dentro = true;
  };

  const salir = () => {
    estado.dentro = false;
    estado.objetivo = null;
    despertar();
  };

  // La onda del clic va sola: nace, crece y se borra con una animación de CSS, que corre
  // fuera del hilo de la página.
  const onda = (e) => {
    const caja = zona.getBoundingClientRect();
    const o = pieza("cr-onda", encima);
    o.style.left = `${e.clientX - caja.left}px`;
    o.style.top = `${e.clientY - caja.top}px`;
    o.addEventListener("animationend", () => o.remove(), { once: true });
    setTimeout(() => o.remove(), 1000); // por si la pestaña estaba escondida y no acabó
  };

  zona.addEventListener("pointermove", (e) => {
    if (e.pointerType !== "mouse") return;
    entrar(e);
    estado.objetivo = elegir(e.target);
    despertar();
  });
  zona.addEventListener("pointerdown", (e) => {
    if (e.pointerType !== "mouse") return;
    entrar(e);
    estado.objetivo = elegir(e.target);
    if (OPCIONES.onda && !directo) onda(e);
    despertar();
  });
  zona.addEventListener("pointerleave", (e) => {
    if (e.pointerType === "mouse") salir();
  });

  // Con la rueda, la página se mueve por debajo de un ratón quieto: lo que hay bajo la
  // punta cambia, y si la cabecera se ha ido hacia arriba, el ratón ya no está en ella.
  window.addEventListener(
    "scroll",
    () => {
      if (!estado.dentro) return;
      const bajo = document.elementFromPoint(estado.tx, estado.ty);
      if (!bajo || !zona.contains(bajo)) return salir();
      estado.objetivo = elegir(bajo);
      despertar();
    },
    { passive: true },
  );
})();
