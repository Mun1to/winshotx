/* Lo que comparten la portada y las páginas de leer (la guía, las novedades, la privacidad):
   las preguntas que se abren de una en una, y el índice de la izquierda, que marca dónde
   estás mientras bajas. Sin dependencias. */

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
