/* Lo que comparten la portada y las páginas de leer (la guía, las novedades, la privacidad):
   las preguntas que se abren de una en una, y el índice de la izquierda, que marca dónde
   estás mientras bajas. Sin dependencias. */

/* ------------------------------------------------- sin tarjeta gráfica, más ligera */
// El modo ligero de la skill frontlaxweb. Sin aceleración por hardware (el ordenador de
// clase de Munito, muchos de empresa y los viejos), el procesador hace lo que hacía la
// tarjeta: cada desenfoque se vuelve a calcular al bajar. Medido sin gráfica y con la CPU a
// 4x (14-nov, el PC cargado): la portada bajaba a 29-32 fotogramas por segundo y la publicada
// a 31-34; el que más pesaba era el cristal de la barra de arriba (sin él, 38-39), y sin
// ningún desenfoque, 40. Se mira UNA vez, después del primer pintado: un contexto WebGL
// con `failIfMajorPerformanceCaveat` se niega (null) cuando solo se dibujaría por software.
// Entonces se pone <html data-ligero> y el CSS quita lo que pesa sin cambiar el diseño.
// Con gráfica no cambia nada. `?ligero=1` lo fuerza y `?ligero=0` lo quita. Las
// animaciones no se tocan: la política de la casa es que se vean siempre.
(function () {
  const pedido = () => {
    for (const v of [window, window.parent]) {
      try {
        const q = new URLSearchParams(v.location.search).get("ligero");
        if (q) return q;
      } catch (e) { /* un marco de otro origen: no se mira */ }
    }
    return null;
  };
  const porSoftware = () => {
    const q = pedido();
    if (q === "1") return true;
    if (q === "0") return false;
    try {
      if (typeof WebGLRenderingContext === "undefined") return false;
      const lienzo = document.createElement("canvas");
      const op = { failIfMajorPerformanceCaveat: true };
      const gl = lienzo.getContext("webgl2", op) || lienzo.getContext("webgl", op);
      if (!gl) return true;
      const soltar = gl.getExtension("WEBGL_lose_context");
      if (soltar) soltar.loseContext();
      return false;
    } catch (e) {
      return false;
    }
  };
  requestAnimationFrame(() => setTimeout(() => {
    if (porSoftware()) document.documentElement.setAttribute("data-ligero", "");
  }, 0));
})();

/* ---------------------------------------------------------------- preguntas */
// Una pregunta abierta a la vez, y el cambio de una a otra en un solo movimiento.
// <details> no da nada de esto solo: deja abrir todas a la vez, y al quitarle `open` borra
// el contenido de golpe, así que la animación de salida no llega a verse. Por eso
// `data-abierta` manda en el CSS y `open` se queda un ratito más mientras dura la
// transición de cierre.
document.querySelectorAll(".faq").forEach((lista) => {
  const items = Array.from(lista.querySelectorAll("details"));
  const ms = parseFloat(getComputedStyle(lista).getPropertyValue("--faq-ms")) || 380;
  const pendientes = new Map();

  const cerrar = (item) => {
    if (!item.open) return;
    item.removeAttribute("data-abierta");
    clearTimeout(pendientes.get(item));
    pendientes.set(
      item,
      setTimeout(() => {
        if (!item.hasAttribute("data-abierta")) item.open = false;
        pendientes.delete(item);
      }, ms),
    );
  };

  const abrir = (item) => {
    clearTimeout(pendientes.get(item));
    pendientes.delete(item);
    item.open = true;
    requestAnimationFrame(() => item.setAttribute("data-abierta", ""));
  };

  items.forEach((item) => {
    item.querySelector("summary").addEventListener("click", (evento) => {
      evento.preventDefault();
      const estaAbierta = item.hasAttribute("data-abierta");
      items.forEach((otra) => { if (otra !== item) cerrar(otra); });
      if (estaAbierta) cerrar(item);
      else abrir(item);
    });
  });
});

/* ------------------------------------------------------- el índice de la izquierda */
// La sección en la que estás es la última cuyo título ya ha pasado del primer tercio de la
// ventana; al llegar al final de la página, la última, aunque sea corta y no llegue ahí.
(function () {
  const indice = document.querySelector(".leer-indice");
  if (!indice) return;
  const enlaces = [...indice.querySelectorAll('a[href^="#"]')];
  const secciones = enlaces.map((a) => document.getElementById(decodeURIComponent(a.hash.slice(1))));
  if (!secciones.length || secciones.some((s) => !s)) return;
  let vivo = null;
  let pendiente = false;

  const marcar = () => {
    pendiente = false;
    const linea = innerHeight * 0.33;
    let cual = 0;
    secciones.forEach((s, i) => { if (s.getBoundingClientRect().top <= linea) cual = i; });
    if (innerHeight + scrollY >= document.documentElement.scrollHeight - 4) cual = secciones.length - 1;
    const enlace = enlaces[cual];
    if (enlace === vivo) return;
    if (vivo) vivo.removeAttribute("aria-current");
    enlace.setAttribute("aria-current", "true");
    vivo = enlace;
    // Si el índice no cabe y baja solo (las novedades son muchas versiones), o en el móvil
    // va de lado, que el marcado se vea. Sin scrollIntoView: movería también la página.
    if (indice.scrollHeight > indice.clientHeight + 1) {
      indice.scrollTop = enlace.offsetTop - indice.clientHeight / 2 + enlace.offsetHeight / 2;
    }
    if (indice.scrollWidth > indice.clientWidth + 1) {
      indice.scrollLeft = enlace.offsetLeft - indice.clientWidth / 2 + enlace.offsetWidth / 2;
    }
  };

  addEventListener("scroll", () => {
    if (pendiente) return;
    pendiente = true;
    requestAnimationFrame(marcar);
  }, { passive: true });
  addEventListener("resize", marcar);
  marcar();
})();

/* ------------------------------------------------- las tablas, apiladas en el móvil */
// Una tabla de cuatro columnas no cabe en 360 px: o se desliza de lado o se lee a dos
// palabras por renglón. En lo estrecho, cada fila pasa a ser una ficha, con su título
// arriba y cada dato en su renglón, y delante de cada dato el nombre de su columna, que
// se saca de la cabecera (así vale igual en la página inglesa). Las de dos columnas
// (tecla y lo que hace) no lo necesitan: se entiende sola.
// El CSS solo las apila si llevan `.apilable`, que la pone esto: sin JS, la tabla se
// queda como estaba, deslizando, pero nunca sin saber de qué columna es cada dato.
// Y como apilarla con CSS le quita a algunos lectores de pantalla la idea de tabla, se
// le dicen los papeles a mano.
document.querySelectorAll(".tabla-comparar, .texto .tabla").forEach((caja) => {
  const tabla = caja.querySelector("table");
  const cabeza = tabla && tabla.tHead && tabla.tHead.rows[0];
  if (!cabeza) return;
  const nombres = [...cabeza.cells].map((c) => c.textContent.trim());
  const papel = (el, p) => { if (!el.hasAttribute("role")) el.setAttribute("role", p); };
  papel(tabla, "table");
  [...tabla.children].forEach((g) => papel(g, "rowgroup"));
  [...tabla.rows].forEach((fila) => {
    papel(fila, "row");
    [...fila.cells].forEach((celda, i) => {
      const enCabeza = fila.parentElement.tagName === "THEAD";
      papel(celda, enCabeza ? "columnheader" : celda.tagName === "TH" ? "rowheader" : "cell");
      if (!enCabeza && celda.tagName === "TD" && nombres.length > 2 && nombres[i]) celda.dataset.col = nombres[i];
    });
  });
  caja.classList.add("apilable");
  if (nombres.length > 2) caja.classList.add("con-etiquetas");
});

/* ----------------------------------------------------------- el menú del móvil */
// Es un <details>, que ya se abre y se cierra solo. Esto lo cierra al elegir una sección
// de esta misma página (si no, se queda abierto encima de lo que has pedido ver), con
// Escape y al tocar fuera de él.
document.querySelectorAll("details.menu").forEach((menu) => {
  const cerrar = () => { menu.open = false; };
  menu.querySelectorAll(".menu-panel a").forEach((a) => a.addEventListener("click", cerrar));
  addEventListener("keydown", (e) => {
    if (e.key === "Escape" && menu.open) { cerrar(); menu.querySelector("summary").focus(); }
  });
  document.addEventListener("click", (e) => { if (menu.open && !menu.contains(e.target)) cerrar(); });
});

/* --------------------------------------------- la barra se compacta al bajar */
// Como la de cattory.co/es: pasados 40 px de scroll, la píldora de arriba se estrecha hasta
// lo justo de lo que lleva dentro (el CSS hace el movimiento, aquí solo se mide y se cambia
// de estado). El scroll solo toca la página cuando el estado cambia, y lo justo se vuelve a
// medir cuando cambia lo que mide algo: el ancho de la ventana o la letra al cargar.
(function () {
  const nav = document.querySelector("nav.barra");
  const pildora = nav && nav.querySelector(".barra-dentro");
  if (!pildora) return;
  const medir = () => {
    const estilo = getComputedStyle(pildora);
    const piezas = [...pildora.children].filter((h) => h.offsetWidth > 0);
    const huecos = parseFloat(estilo.columnGap) * (piezas.length - 1);
    const relleno = parseFloat(estilo.paddingLeft) + parseFloat(estilo.paddingRight);
    // + 8 px de holgura para que el botón de descargar no se parta en dos renglones.
    const justo = piezas.reduce((total, h) => total + h.getBoundingClientRect().width, 0) + huecos + relleno + 8;
    pildora.style.setProperty("--justo", `${Math.ceil(justo)}px`);
  };
  let compacta = null;
  const mirar = () => {
    const toca = scrollY > 40;
    if (toca === compacta) return;
    compacta = toca;
    nav.classList.toggle("compacta", toca);
  };
  medir();
  mirar();
  // Colocada ya como toca, se enciende la transición (en el siguiente fotograma, o el
  // primer estado también se animaría).
  requestAnimationFrame(() => requestAnimationFrame(() => nav.classList.add("anima")));
  addEventListener("scroll", mirar, { passive: true });
  addEventListener("resize", medir);
  addEventListener("load", medir);
  if (document.fonts) document.fonts.ready.then(medir);
})();

/* ------------------------------------- las flechas de las notas de la portada */
// Cada nota con `data-flecha` señala un texto: la de «114 ms» el final de «parpadees», la
// de los disquetes el principio de «Instalador de 2,97 MB». Antes eran dibujos fijos y la
// punta iba girada respecto al trazo, separada de él y, la de los disquetes, 150 px antes
// de lo que señalaba («esto sigue roto, esta flecha bro omg», Munito, 14-nov). Ahora se
// miden la nota y el texto en la página y se traza una curva de rotulador entre los dos;
// la punta se abre desde el final exacto del trazo, en la dirección en la que llega.
// `data-sale`: por dónde sale de la nota (arriba o derecha). `data-curva`: cuánto se
// comba, en partes del largo, y hacia qué lado (con signo). Solo se vuelve a medir cuando
// cambia el tamaño de la cabecera o llega la letra; el scroll no la toca.
// El halo que corta la cruceta por detrás tiene que ser invisible: no puede ser un color
// liso, porque detrás está la luz azul del titular (`.portada::before`) y un liso se veía
// como un contorno. Se pinta dos veces: el fondo de la página y, encima, esa misma luz,
// copiada de su CSS en un degradado del SVG con las mismas medidas.
(function () {
  const portada = document.querySelector(".portada");
  const lienzo = portada && portada.querySelector(".garabatos");
  if (!lienzo) return;
  const NS = "http://www.w3.org/2000/svg";
  const nuevo = (tipo, atributos) => {
    const el = document.createElementNS(NS, tipo);
    for (const [k, v] of Object.entries(atributos)) el.setAttribute(k, v);
    return el;
  };
  const luz = nuevo("radialGradient", { id: "garabatos-luz", gradientUnits: "userSpaceOnUse" });
  const paradaFin = nuevo("stop", { class: "luz-fin" });
  luz.append(nuevo("stop", { offset: "0", class: "luz" }), paradaFin);
  const defs = nuevo("defs", {});
  defs.append(luz);
  lienzo.append(defs);
  const flechas = [...document.querySelectorAll(".portada .fan[data-flecha]")].map((nota) => {
    const halo = nuevo("path", { class: "halo" });
    const haloLuz = nuevo("path", { class: "halo-luz", stroke: "url(#garabatos-luz)" });
    const trazo = nuevo("path", { class: "trazo" });
    lienzo.append(halo, haloLuz, trazo);
    return { nota, caminos: [halo, haloLuz, trazo], destino: document.querySelector(nota.dataset.flecha) };
  });

  // «radial-gradient(58% 48% at 50% 32%, <color>, transparent 72%)» sobre la caja del ::before.
  const copiarLuz = (base) => {
    const antes = getComputedStyle(portada, "::before");
    const m = antes.backgroundImage.match(/radial-gradient\(([\d.]+)% ([\d.]+)% at ([\d.]+)% ([\d.]+)%.*\s([\d.]+)%\)$/);
    if (!m) return false;
    const p = portada.getBoundingClientRect();
    const ancho = p.width;
    const alto = parseFloat(antes.height);
    const arriba = p.top + parseFloat(antes.top) - base.top;
    const [rx, ry, cx, cy, hasta] = m.slice(1).map((v) => parseFloat(v) / 100);
    const x = p.left - base.left + cx * ancho;
    const y = arriba + cy * alto;
    luz.setAttribute("cx", x);
    luz.setAttribute("cy", y);
    luz.setAttribute("r", rx * ancho);
    luz.setAttribute("gradientTransform", `translate(${x} ${y}) scale(1 ${(ry * alto) / (rx * ancho)}) translate(${-x} ${-y})`);
    paradaFin.setAttribute("offset", hasta);
    return true;
  };

  // La caja de la primera o de la última letra de un texto, sin los espacios de los lados.
  const letra = (el, alFinal) => {
    const textos = [];
    const paseo = document.createTreeWalker(el, NodeFilter.SHOW_TEXT);
    for (let n = paseo.nextNode(); n; n = paseo.nextNode()) if (n.data.trim()) textos.push(n);
    if (!textos.length) return null;
    const n = alFinal ? textos[textos.length - 1] : textos[0];
    const i = alFinal ? n.data.trimEnd().length - 1 : n.data.search(/\S/);
    const rango = document.createRange();
    rango.setStart(n, i);
    rango.setEnd(n, i + 1);
    return rango.getBoundingClientRect();
  };

  const dibujar = () => {
    // Primero se lee todo y luego se escribe, para no recalcular la página en cada flecha.
    if (getComputedStyle(lienzo).display === "none") return;
    const base = lienzo.getBoundingClientRect();
    lienzo.classList.toggle("con-luz", copiarLuz(base));
    const medidas = flechas.map((f) => {
      const t = f.destino && letra(f.destino, f.nota.dataset.punta === "final");
      const n = f.nota.firstElementChild.getBoundingClientRect();
      return { f, t, n, cuerpo: parseFloat(getComputedStyle(f.nota).fontSize) || 22 };
    });
    for (const { f, t, n, cuerpo } of medidas) {
      if (!t) continue;
      const hueco = cuerpo * 0.5;
      const alFinal = f.nota.dataset.punta === "final";
      // La punta, a media altura de las minúsculas, separada de la letra.
      const fin = [(alFinal ? t.right + hueco : t.left - hueco) - base.left, t.top + t.height * 0.58 - base.top];
      const ini = f.nota.dataset.sale === "arriba"
        ? [n.left + n.width * 0.42 - base.left, n.top - hueco * 0.8 - base.top]
        : [n.right + hueco * 0.8 - base.left, n.top + n.height * 0.42 - base.top];
      const d = [fin[0] - ini[0], fin[1] - ini[1]];
      const largo = Math.hypot(d[0], d[1]);
      if (largo < cuerpo * 2) { f.caminos.forEach((c) => c.removeAttribute("d")); continue; }
      const normal = [-d[1] / largo, d[0] / largo];
      const k = parseFloat(f.nota.dataset.curva) || 0;
      // Un poco más combada al salir que al llegar, como cuando se hace de un tirón.
      const c1 = [ini[0] + d[0] * 0.28 + normal[0] * k * 1.15 * largo, ini[1] + d[1] * 0.28 + normal[1] * k * 1.15 * largo];
      const c2 = [ini[0] + d[0] * 0.7 + normal[0] * k * 0.85 * largo, ini[1] + d[1] * 0.7 + normal[1] * k * 0.85 * largo];
      // La dirección en la que llega el trazo, y hacia atrás desde ahí, las dos patas de la
      // punta: algo distintas entre sí, como a mano, pero las dos desde el mismo punto.
      const atras = Math.atan2(c2[1] - fin[1], c2[0] - fin[0]);
      const pata = (giro, largoPata) => [fin[0] + Math.cos(atras + giro) * largoPata, fin[1] + Math.sin(atras + giro) * largoPata];
      const p1 = pata(0.47, cuerpo * 0.56);
      const p2 = pata(-0.56, cuerpo * 0.5);
      const r = (p) => `${p[0].toFixed(1)} ${p[1].toFixed(1)}`;
      const camino = `M${r(ini)}C${r(c1)} ${r(c2)} ${r(fin)}M${r(p1)}L${r(fin)}L${r(p2)}`;
      f.caminos.forEach((c) => c.setAttribute("d", camino));
    }
  };
  dibujar();
  new ResizeObserver(dibujar).observe(document.querySelector(".portada > .env"));
  addEventListener("load", dibujar);
  if (document.fonts) document.fonts.ready.then(dibujar);
})();

/* ------------------------------------------- las órdenes se copian con un clic */
// Una orden para pegar en la terminal (winget, la de añadirlo a Claude Code, las del
// agente) se pincha entera y va al portapapeles, con su aviso de copiado ahí mismo. Lo
// pidió Munito delante de la portada: «cuando le des click se copie, ¿no?». Vale con el
// teclado (Tab y Intro o espacio) y en los dos idiomas. Sin JS se queda como texto que se
// puede seleccionar a mano, que es lo que había.
// Qué es una orden: un <code> que empieza por un programa de la terminal, cada línea
// `.orden` de la terminal dibujada (sin su «PS>», que no se escribe) y el bloque de una
// orden larga de la guía (`.orden-bloque`). Las dos últimas van en fila, con el aviso a la
// derecha; las de dentro de una frase, con el aviso encima, que va suelto en la página
// para que no lo corte el cajón de una pregunta.
(function () {
  const ingles = document.documentElement.lang === "en";
  const T = ingles
    ? { copiar: "Copy", copiado: "Copied", fallo: "Selected: press Ctrl+C" }
    : { copiar: "Copiar", copiado: "Copiado", fallo: "Seleccionado: pulsa Ctrl+C" };
  const ORDEN = /^(winget|winshotx(\.exe)?|claude)\s/;
  const dibujo = (clase, trazo) =>
    `<svg class="${clase}" viewBox="0 0 24 24" aria-hidden="true" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">${trazo}</svg>`;
  const ICONOS =
    dibujo("ico-copiar", '<rect width="14" height="14" x="8" y="8" rx="2"/><path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"/>') +
    dibujo("ico-hecho", '<path d="M20 6 9 17l-5-5"/>');

  const piezas = [
    ...[...document.querySelectorAll("code")].filter((c) => ORDEN.test(c.textContent.trim()) && !c.closest(".orden")),
    ...document.querySelectorAll(".terminal .orden"),
  ];
  if (!piezas.length) return;

  // Lo que oye un lector de pantalla al copiar: un solo aviso para toda la página.
  const oido = document.createElement("span");
  oido.className = "solo-lector";
  oido.setAttribute("aria-live", "polite");
  // El aviso de las órdenes de dentro de una frase: uno para todas, en la página.
  const globo = document.createElement("span");
  globo.className = "copiado copiado-globo";
  globo.setAttribute("aria-hidden", "true");
  document.body.append(oido, globo);
  let quitarGlobo = 0;

  /** Al portapapeles; si el navegador no deja, se selecciona para copiarla a mano. */
  async function copiar(texto, pieza) {
    try {
      await navigator.clipboard.writeText(texto);
      return true;
    } catch {
      const area = document.createElement("textarea");
      area.value = texto;
      area.setAttribute("readonly", "");
      area.style.cssText = "position:fixed;top:0;left:0;opacity:0;pointer-events:none";
      document.body.appendChild(area);
      area.select();
      let hecho = false;
      try { hecho = document.execCommand("copy"); } catch { hecho = false; }
      area.remove();
      if (hecho) return true;
      // Solo el texto de la orden, sin el dibujo ni el aviso que lleva detrás.
      const rango = document.createRange();
      rango.setStart(pieza, 0);
      rango.setEndBefore(pieza.querySelector(".ico-copiar"));
      getSelection().removeAllRanges();
      getSelection().addRange(rango);
      return false;
    }
  }

  /** El aviso encima del renglón de la orden que se ha pulsado, sin salirse de la pantalla. */
  function ponerGlobo(pieza, frase, punto) {
    const trozos = [...pieza.getClientRects()];
    const r = (punto && trozos.find((q) => punto.y >= q.top && punto.y <= q.bottom)) || trozos[0];
    globo.textContent = frase;
    const medio = globo.offsetWidth / 2 + 8;
    const x = Math.min(innerWidth - medio, Math.max(medio, r.left + r.width / 2));
    globo.style.left = `${x + scrollX}px`;
    globo.style.top = `${r.top + scrollY}px`;
    globo.dataset.visible = "";
    clearTimeout(quitarGlobo);
    quitarGlobo = setTimeout(() => delete globo.dataset.visible, 1600);
  }

  piezas.forEach((pieza) => {
    const sinPrompt = pieza.cloneNode(true);
    sinPrompt.querySelectorAll(".ps").forEach((p) => p.remove());
    const texto = sinPrompt.textContent.replace(/\s+/g, " ").trim();
    const enFila = pieza.matches(".terminal .orden, .orden-bloque code");
    pieza.classList.add("copiable", enFila ? "en-fila" : "en-frase");
    pieza.setAttribute("role", "button");
    pieza.tabIndex = 0;
    pieza.setAttribute("aria-label", `${T.copiar}: ${texto}`);
    pieza.title = T.copiar;
    pieza.insertAdjacentHTML("beforeend", ICONOS);
    const aviso = enFila ? document.createElement("span") : null;
    if (aviso) {
      aviso.className = "copiado";
      aviso.setAttribute("aria-hidden", "true");
      pieza.appendChild(aviso);
    }
    let quitar = 0;

    const hacer = async (punto) => {
      const bien = await copiar(texto, pieza);
      const frase = bien ? T.copiado : T.fallo;
      if (aviso) aviso.textContent = frase;
      else ponerGlobo(pieza, frase, punto);
      oido.textContent = "";
      oido.textContent = bien ? `${T.copiado}: ${texto}` : T.fallo;
      pieza.dataset.copiado = bien ? "si" : "no";
      clearTimeout(quitar);
      quitar = setTimeout(() => delete pieza.dataset.copiado, bien ? 1600 : 3200);
    };
    pieza.addEventListener("click", (e) => {
      // Si alguien está seleccionando un trozo a mano, no se le pisa la selección.
      if (String(getSelection()).length && pieza.contains(getSelection().anchorNode)) return;
      e.preventDefault();
      hacer({ x: e.clientX, y: e.clientY });
    });
    pieza.addEventListener("keydown", (e) => {
      if (e.key !== "Enter" && e.key !== " ") return;
      e.preventDefault();
      hacer(null);
    });
  });
})();
