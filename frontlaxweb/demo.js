/* Demo de winshotx dentro de la página: un escritorio falso sobre el que se recorta de
   verdad, el editor con su tira de fotogramas y el panel de ajustes. Sin dependencias. */

/* ---------------------------------------------------------------- pestañas */
document.querySelectorAll(".pestana").forEach((boton) => {
  boton.addEventListener("click", () => {
    document.querySelectorAll(".pestana").forEach((otro) => {
      const activo = otro === boton;
      otro.setAttribute("aria-selected", String(activo));
      document.getElementById(otro.dataset.panel).hidden = !activo;
    });
  });
});

/* ------------------------------------------------------- palancas de ajustes */
document.querySelectorAll(".palanca").forEach((p) => {
  p.addEventListener("click", () => (p.dataset.on = p.dataset.on === "1" ? "0" : "1"));
});

/* ------------------------------------------------------- el escritorio falso */
const ANCHO = 1280;
const ALTO = 800;
const escritorio = document.getElementById("escritorio");
escritorio.width = ANCHO;
escritorio.height = ALTO;
const pincel = escritorio.getContext("2d", { willReadFrequently: true });

function ventana(c, x, y, w, h, titulo, tono) {
  c.fillStyle = "#00000055";
  c.beginPath();
  c.roundRect(x + 6, y + 10, w, h, 12);
  c.fill();
  c.fillStyle = "#1b1b21";
  c.beginPath();
  c.roundRect(x, y, w, h, 12);
  c.fill();
  c.fillStyle = "#25252d";
  c.beginPath();
  c.roundRect(x, y, w, 34, [12, 12, 0, 0]);
  c.fill();
  ["#ff5f57", "#febc2e", "#28c840"].forEach((color, i) => {
    c.fillStyle = color;
    c.beginPath();
    c.arc(x + 18 + i * 17, y + 17, 5, 0, Math.PI * 2);
    c.fill();
  });
  c.fillStyle = "#8b8b98";
  c.font = "13px ui-sans-serif, system-ui, sans-serif";
  c.fillText(titulo, x + 78, y + 22);
  // contenido: renglones de distinto ancho, como cualquier ventana con texto
  let fila = y + 58;
  for (let i = 0; fila < y + h - 22; i++, fila += 22) {
    c.fillStyle = i % 4 === 0 ? tono : "#ffffff14";
    const ancho = i % 4 === 0 ? 120 : 60 + ((i * 83) % (w - 130));
    c.beginPath();
    c.roundRect(x + 20, fila, Math.min(ancho, w - 40), 10, 5);
    c.fill();
  }
}

function pintarEscritorio() {
  const cielo = pincel.createLinearGradient(0, 0, ANCHO, ALTO);
  cielo.addColorStop(0, "#0d2a4a");
  cielo.addColorStop(0.5, "#10365e");
  cielo.addColorStop(1, "#071a2e");
  pincel.fillStyle = cielo;
  pincel.fillRect(0, 0, ANCHO, ALTO);

  const halo = pincel.createRadialGradient(340, 200, 20, 340, 200, 520);
  halo.addColorStop(0, "#0a9bff3d");
  halo.addColorStop(1, "#0a9bff00");
  pincel.fillStyle = halo;
  pincel.fillRect(0, 0, ANCHO, ALTO);

  ventana(pincel, 90, 110, 520, 400, "informe.md", "#0a9bff");
  ventana(pincel, 520, 300, 640, 420, "terminal", "#3ddc84");

  // barra de tareas
  pincel.fillStyle = "#0c0c11d9";
  pincel.fillRect(0, ALTO - 52, ANCHO, 52);
  for (let i = 0; i < 6; i++) {
    pincel.fillStyle = ["#0a9bff", "#3ddc84", "#febc2e", "#ff5f57", "#a78bfa", "#8b8b98"][i];
    pincel.globalAlpha = 0.85;
    pincel.beginPath();
    pincel.roundRect(ANCHO / 2 - 150 + i * 52, ALTO - 39, 26, 26, 7);
    pincel.fill();
    pincel.globalAlpha = 1;
  }
}
pintarEscritorio();

/* ------------------------------------------------------------- la selección */
const caja = document.getElementById("caja");
const velo = document.getElementById("velo");
const sel = document.getElementById("sel");
const medida = document.getElementById("medida");
const lupa = document.getElementById("lupa");
const zoom = document.getElementById("zoom");
const hex = document.getElementById("hex");
const herramientas = document.getElementById("herramientas");
const pista = document.getElementById("pista");
const aviso = document.getElementById("aviso");
const lente = zoom.getContext("2d");
lente.imageSmoothingEnabled = false;

let arrastrando = false;
let hay = false;
let ini = { x: 0, y: 0 };
let fin = { x: 0, y: 0 };

const enCaja = (e) => {
  const r = caja.getBoundingClientRect();
  return {
    x: Math.max(0, Math.min(1, (e.clientX - r.left) / r.width)),
    y: Math.max(0, Math.min(1, (e.clientY - r.top) / r.height)),
  };
};

function rectangulo() {
  return {
    x: Math.min(ini.x, fin.x),
    y: Math.min(ini.y, fin.y),
    w: Math.abs(fin.x - ini.x),
    h: Math.abs(fin.y - ini.y),
  };
}

function pintarSeleccion() {
  const r = rectangulo();
  const pct = (v) => `${v * 100}%`;
  sel.style.cssText += `display:block;left:${pct(r.x)};top:${pct(r.y)};width:${pct(r.w)};height:${pct(r.h)}`;
  sel.style.boxShadow = "0 0 0 9999px #000000a6";
  velo.style.display = "none";
  const px = Math.round(r.w * ANCHO);
  const py = Math.round(r.h * ALTO);
  medida.style.display = "block";
  medida.textContent = `${px} × ${py}`;
  const arriba = r.y * caja.clientHeight > 30;
  medida.style.left = `${r.x * 100}%`;
  medida.style.top = arriba
    ? `calc(${r.y * 100}% - 26px)`
    : `calc(${(r.y + r.h) * 100}% + 8px)`;
}

function pintarLupa(p) {
  const lado = 116 / 6; // 6 aumentos, como la app
  lente.clearRect(0, 0, 116, 116);
  lente.drawImage(
    escritorio,
    p.x * ANCHO - lado / 2, p.y * ALTO - lado / 2, lado, lado,
    0, 0, 116, 116,
  );
  lente.strokeStyle = "#ffffff1a";
  lente.lineWidth = 1;
  for (let i = 6; i < 116; i += 6) {
    lente.beginPath();
    lente.moveTo(i + 0.5, 0);
    lente.lineTo(i + 0.5, 116);
    lente.moveTo(0, i + 0.5);
    lente.lineTo(116, i + 0.5);
    lente.stroke();
  }
  lente.strokeStyle = "#0a9bff";
  lente.lineWidth = 2;
  lente.strokeRect(55, 55, 7, 7);

  const d = pincel.getImageData(Math.round(p.x * ANCHO), Math.round(p.y * ALTO), 1, 1).data;
  const codigo = "#" + [d[0], d[1], d[2]].map((v) => v.toString(16).padStart(2, "0")).join("");
  hex.textContent = codigo.toUpperCase();
  lupa.style.display = "block";
  const derecha = p.x > 0.8;
  const abajo = p.y > 0.75;
  lupa.style.left = derecha ? `calc(${p.x * 100}% - 136px)` : `calc(${p.x * 100}% + 20px)`;
  lupa.style.top = abajo ? `calc(${p.y * 100}% - 150px)` : `calc(${p.y * 100}% + 20px)`;
}

function colocarHerramientas() {
  const r = rectangulo();
  herramientas.style.display = "flex";
  const cabeAbajo = (r.y + r.h) * caja.clientHeight + 60 < caja.clientHeight;
  herramientas.style.left = `calc(${(r.x + r.w) * 100}% - 232px)`;
  herramientas.style.top = cabeAbajo
    ? `calc(${(r.y + r.h) * 100}% + 10px)`
    : `calc(${r.y * 100}% - 54px)`;
}

function limpiar() {
  hay = false;
  arrastrando = false;
  sel.style.display = "none";
  medida.style.display = "none";
  herramientas.style.display = "none";
  velo.style.display = "block";
  pista.style.display = "block";
  aviso.style.display = "none";
}

caja.addEventListener("pointerdown", (e) => {
  if (e.target.closest(".herramientas")) return;
  caja.setPointerCapture(e.pointerId);
  arrastrando = true;
  hay = false;
  herramientas.style.display = "none";
  aviso.style.display = "none";
  pista.style.display = "none";
  ini = fin = enCaja(e);
  pintarSeleccion();
});

caja.addEventListener("pointermove", (e) => {
  const p = enCaja(e);
  // La pista sobra en cuanto el ratón entra: si no, la lupa se le pone encima.
  pista.style.display = "none";
  if (arrastrando) {
    fin = p;
    pintarSeleccion();
  }
  if (!hay) pintarLupa(p);
});

caja.addEventListener("pointerup", (e) => {
  if (!arrastrando) return;
  arrastrando = false;
  fin = enCaja(e);
  const r = rectangulo();
  if (r.w * ANCHO < 24 || r.h * ALTO < 24) return limpiar();
  hay = true;
  lupa.style.display = "none";
  pintarSeleccion();
  colocarHerramientas();
});

caja.addEventListener("pointerleave", () => {
  if (!arrastrando && !hay) lupa.style.display = "none";
});

herramientas.addEventListener("click", (e) => {
  const boton = e.target.closest("button");
  if (!boton) return;
  const accion = boton.dataset.accion;
  if (accion === "cancelar") return limpiar();
  const r = rectangulo();
  aviso.style.display = "flex";
  aviso.querySelector("span").textContent =
    `${accion} · ${Math.round(r.w * ANCHO)} × ${Math.round(r.h * ALTO)} px`;
});

document.addEventListener("keydown", (e) => {
  if (e.key === "Escape" && (hay || arrastrando)) limpiar();
});

/* ------------------------------------------------------------- el editor */
const TOTAL = 24;
const lienzoEditor = document.getElementById("lienzo-editor");
lienzoEditor.width = 960;
lienzoEditor.height = 480;
const brocha = lienzoEditor.getContext("2d");
const tira = document.getElementById("tira");
const estadoEditor = document.getElementById("estado-editor");
const botonPlay = document.getElementById("reproducir");

let actual = 0;
let dentro = 0;
let fuera = TOTAL - 1;
let reproduciendo = null;

/** Un fotograma de una interacción cualquiera: el cursor llega, pulsa y el botón responde. */
function fotograma(c, w, h, i) {
  const t = i / (TOTAL - 1);
  c.fillStyle = "#101018";
  c.fillRect(0, 0, w, h);
  c.fillStyle = "#181820";
  c.beginPath();
  c.roundRect(w * 0.08, h * 0.14, w * 0.84, h * 0.72, 14);
  c.fill();

  c.fillStyle = "#2a2a36";
  for (let f = 0; f < 3; f++) {
    c.beginPath();
    c.roundRect(w * 0.13, h * 0.24 + f * h * 0.1, w * (0.5 - f * 0.11), h * 0.045, 6);
    c.fill();
  }

  const pulsado = t > 0.62;
  c.fillStyle = pulsado ? "#0a9bff" : "#1f6fb0";
  c.beginPath();
  c.roundRect(w * 0.13, h * 0.62, w * 0.26, h * 0.13, 10);
  c.fill();
  c.fillStyle = "#fff";
  c.font = `600 ${Math.round(h * 0.055)}px ui-sans-serif, system-ui, sans-serif`;
  c.fillText("Exportar", w * 0.17, h * 0.705);

  if (pulsado) {
    c.strokeStyle = `rgba(10,155,255,${1 - (t - 0.62) / 0.38})`;
    c.lineWidth = 3;
    c.beginPath();
    c.roundRect(
      w * 0.13 - (t - 0.62) * 60, h * 0.62 - (t - 0.62) * 60,
      w * 0.26 + (t - 0.62) * 120, h * 0.13 + (t - 0.62) * 120, 16,
    );
    c.stroke();
  }

  // el cursor viaja hasta el botón y se queda
  const destino = { x: w * 0.24, y: h * 0.69 };
  const salida = { x: w * 0.78, y: h * 0.26 };
  const avance = Math.min(1, t / 0.62);
  const suave = 1 - Math.pow(1 - avance, 3);
  const cx = salida.x + (destino.x - salida.x) * suave;
  const cy = salida.y + (destino.y - salida.y) * suave;
  c.fillStyle = "#fff";
  c.strokeStyle = "#000";
  c.lineWidth = 1.5;
  c.beginPath();
  c.moveTo(cx, cy);
  c.lineTo(cx, cy + h * 0.075);
  c.lineTo(cx + w * 0.014, cy + h * 0.055);
  c.lineTo(cx + w * 0.026, cy + h * 0.078);
  c.lineTo(cx + w * 0.034, cy + h * 0.072);
  c.lineTo(cx + w * 0.022, cy + h * 0.049);
  c.lineTo(cx + w * 0.038, cy + h * 0.046);
  c.closePath();
  c.fill();
  c.stroke();
}

function refrescarEditor() {
  fotograma(brocha, lienzoEditor.width, lienzoEditor.height, actual);
  [...tira.children].forEach((b, i) => {
    b.dataset.dentro = i >= dentro && i <= fuera ? "1" : "0";
    b.dataset.actual = i === actual ? "1" : "0";
  });
  const n = fuera - dentro + 1;
  const seg = (n / 30).toFixed(2).replace(".", ",");
  estadoEditor.textContent =
    `Recorte ${dentro + 1} a ${fuera + 1} · ${n} fotograma${n === 1 ? "" : "s"} · ${seg} s`;
}

for (let i = 0; i < TOTAL; i++) {
  const boton = document.createElement("button");
  boton.title = `Fotograma ${i + 1}`;
  const mini = document.createElement("canvas");
  mini.width = 104;
  mini.height = 76;
  fotograma(mini.getContext("2d"), 104, 76, i);
  boton.appendChild(mini);
  boton.addEventListener("click", () => {
    actual = i;
    refrescarEditor();
  });
  tira.appendChild(boton);
}

botonPlay.addEventListener("click", () => {
  if (reproduciendo) {
    clearInterval(reproduciendo);
    reproduciendo = null;
    botonPlay.textContent = "▶ Reproducir";
    return;
  }
  botonPlay.textContent = "❚❚ Pausa";
  actual = dentro;
  reproduciendo = setInterval(() => {
    actual = actual >= fuera ? dentro : actual + 1;
    refrescarEditor();
  }, 1000 / 15);
});

document.querySelectorAll("[data-marca]").forEach((b) => {
  b.addEventListener("click", () => {
    if (b.dataset.marca === "in") dentro = Math.min(actual, fuera);
    else fuera = Math.max(actual, dentro);
    refrescarEditor();
  });
});

refrescarEditor();
