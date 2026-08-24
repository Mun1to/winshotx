/**
 * El contador de estrellas del boton de GitHub. Va aparte de demo.js porque la guia
 * lo necesita y la demo no existe alli.
 */
// El contador de estrellas: se pide a GitHub y, si no contesta o va lento, se queda
// el numero que ya venia escrito en el HTML. Nunca deja el hueco vacio ni un cero.
(async () => {
  const hueco = document.getElementById("gh-estrellas");
  if (!hueco) return;
  try {
    const r = await fetch("https://api.github.com/repos/Mun1to/winshotx", {
      headers: { Accept: "application/vnd.github+json" },
    });
    if (!r.ok) return;
    const { stargazers_count: estrellas } = await r.json();
    // Cero pasaria el typeof y borraria el numero escrito a mano, que es justo lo que
    // este bloque promete no hacer.
    if (typeof estrellas === "number" && estrellas > 0) {
      hueco.textContent = estrellas >= 1000 ? (estrellas / 1000).toFixed(1) + "k" : estrellas;
    }
  } catch {
    /* sin red o con la cuota agotada: se queda el numero de siempre */
  }
})();
