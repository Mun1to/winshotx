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
