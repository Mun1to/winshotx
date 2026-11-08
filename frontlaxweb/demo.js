/* winshotx dentro de la página. Las dos pantallas están rehechas a partir del código de la
   app: mismas medidas, mismos textos y los mismos iconos de lucide. Sin dependencias. */

/* ------------------------------------------------- lo que comparte con la app */
const AZUL_APP = "#3b82f6";

function formatDuration(ms) {
  const total = Math.max(0, Math.round(ms / 1000));
  return `${Math.floor(total / 60)}:${String(total % 60).padStart(2, "0")}`;
}
function formatTimecode(ms) {
  const centis = Math.floor((ms % 1000) / 10);
  return `${formatDuration(Math.floor(ms / 1000) * 1000)}.${String(centis).padStart(2, "0")}`;
}
function formatBytes(bytes) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`;
  // La coma es la separacion decimal en espanol y el punto en ingles: se decide con el
  // idioma que declara la propia pagina, no a mano.
  const decimal = document.documentElement.lang === "en" ? "." : ",";
  return `${(bytes / (1024 * 1024)).toFixed(1).replace(".", decimal)} MB`;
}
function plural(cantidad, singular, terminacion = "s") {
  return `${cantidad} ${singular}${cantidad === 1 ? "" : terminacion}`;
}
const clamp = (v, min, max) => Math.min(max, Math.max(min, v));

/* --------------------------------------------------------------- idiomas */
/* El botón dice a qué idioma cambias: si pone English, la página está en español. */
const FRASES = {
  teQuedasCon: ["Te quedas con {trozo} de {total}", "Keeping {trozo} of {total}"],
  arrastra: ["Arrastra los bordes azules para recortar", "Drag the blue edges to trim"],
  principio: ["Principio: arrástralo para recortar (tecla I)", "Start: drag it to trim (key I)"],
  final: ["Final: arrástralo para recortar (tecla O)", "End: drag it to trim (key O)"],
  reproducir: ["Reproducir", "Play"],
  pausar: ["Pausar", "Pause"],
  paraMp4: ["Vídeo con sonido. Se abre en cualquier sitio.", "Video with sound. Opens anywhere."],
  paraGif: ["Se repite solo y sin sonido. Para pegarlo en un chat.", "Loops on its own, with no sound. For pasting into a chat."],
  paraPng: ["Solo el fotograma que estás viendo, sin perder nada.", "Just the frame you are looking at, with nothing lost."],
  paraJpg: ["Solo el fotograma que estás viendo, mucho más ligero.", "Just the frame you are looking at, much lighter."],
  guardado: ["Guardado", "Saved"],
  copiar: ["Copiada al portapapeles", "Copied to the clipboard"],
  guardar: ["Guardada en Pictures\\winshotx", "Saved to Pictures\\winshotx"],
  editar: ["Abriendo el editor", "Opening the editor"],
  gif: ["Grabando un GIF de esa región", "Recording a GIF of that region"],
  video: ["Grabando vídeo de esa región", "Recording video of that region"],
  pantallaFoto: ["Pantalla entera capturada", "Whole screen captured"],
  pantallaVideo: ["Grabando la pantalla entera", "Recording the whole screen"],
  pantallaGif: ["Grabando la pantalla entera en GIF", "Recording the whole screen to GIF"],
  cartelFoto: ["o clic para capturarla entera", "or click to capture it whole"],
  cartelVideo: ["o clic para grabarla entera", "or click to record it whole"],
  cartelGif: ["o clic para grabarla entera en GIF", "or click to record it whole to GIF"],
  copiado: ["copiado", "copied"],
  abrirCarpeta: ["abrir carpeta", "open folder"],
  exportar: ["Exportar", "Export"],
  bloqueada: ["Proporción bloqueada", "Aspect ratio locked"],
  libre: ["Proporción libre", "Aspect ratio free"],
};
/* Cada idioma tiene su propia URL, así que aquí solo hay que leer cuál es esta página. */
const idioma = document.documentElement.lang === "en" ? "en" : "es";
const frase = (clave) => FRASES[clave][idioma === "en" ? 1 : 0];

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
// Las cuatro pestannas de dentro de ajustes: capturar, grabar, teclas y la app. Son las
// mismas que tiene la ventana de verdad, asi que se pasa de una a otra igual que alli.
document.querySelectorAll(".pes-set button").forEach((boton) => {
  boton.addEventListener("click", () => {
    document.querySelectorAll(".pes-set button").forEach((otro) => {
      const activo = otro === boton;
      otro.classList.toggle("viva", activo);
      document.getElementById(otro.dataset.set).hidden = !activo;
    });
  });
});

// Los segmentados (fotogramas, calidad, tema, idioma...) y la barra de abajo: al pulsar
// uno, se enciende ese y se apagan sus hermanos. Sin esto la demo se ve pero no responde.
document.querySelectorAll(".segmentado, .mini-seg").forEach((grupo) => {
  grupo.querySelectorAll("button").forEach((boton) => {
    boton.addEventListener("click", () => {
      grupo.querySelectorAll("button").forEach((otro) => {
        otro.classList.toggle("viva", otro === boton);
      });
    });
  });
});

document.querySelectorAll(".palanca").forEach((p) => {
  p.addEventListener("click", () => {
    p.dataset.on = p.dataset.on === "1" ? "0" : "1";
    // Sin esto el lector de pantalla sigue anunciando el estado anterior.
    p.setAttribute("aria-checked", p.dataset.on === "1" ? "true" : "false");
  });
});

/* ======================================================= 1. seleccionar === */
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
  let fila = y + 58;
  for (let i = 0; fila < y + h - 22; i++, fila += 22) {
    c.fillStyle = i % 4 === 0 ? tono : "#ffffff14";
    const ancho = i % 4 === 0 ? 120 : 60 + ((i * 83) % (w - 130));
    c.beginPath();
    c.roundRect(x + 20, fila, Math.min(ancho, w - 40), 10, 5);
    c.fill();
  }
}

(function pintarEscritorio() {
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

  pincel.fillStyle = "#0c0c11d9";
  pincel.fillRect(0, ALTO - 52, ANCHO, 52);
  ["#0a9bff", "#3ddc84", "#febc2e", "#ff5f57", "#a78bfa", "#8b8b98"].forEach((color, i) => {
    pincel.fillStyle = color;
    pincel.globalAlpha = 0.85;
    pincel.beginPath();
    pincel.roundRect(ANCHO / 2 - 150 + i * 52, ALTO - 39, 26, 26, 7);
    pincel.fill();
    pincel.globalAlpha = 1;
  });
})();

const caja = document.getElementById("caja");
const velo = document.getElementById("velo");
const sel = document.getElementById("sel");
const cajaTiradores = document.getElementById("tiradores");
const medida = document.getElementById("medida");
const lupa = document.getElementById("lupa");
const hex = document.getElementById("hex");
const herramientas = document.getElementById("herramientas");
const modos = document.getElementById("modos");
const eligePantalla = document.getElementById("elige-pantalla");
const cartelPantalla = document.getElementById("cartel-pantalla");
const botonGrabar = herramientas.querySelector(".grabar");
/* Qué sale del recorte. Se elige arriba, antes de recortar, igual que en la app. */
let modo = "foto";
let pantallaEntera = false;
const aviso = document.getElementById("aviso");
const lente = document.getElementById("zoom").getContext("2d");
lente.imageSmoothingEnabled = false;

// los ocho tiradores de SelectionHandles: esquinas y puntos medios
const PUNTOS = [
  [0, 0], [0.5, 0], [1, 0],
  [0, 0.5], [1, 0.5],
  [0, 1], [0.5, 1], [1, 1],
];
PUNTOS.forEach(() => cajaTiradores.appendChild(document.createElement("i")));

let arrastrando = false;
let hay = false;
let ini = { x: 0, y: 0 };
let fin = { x: 0, y: 0 };

const enCaja = (e) => {
  const r = caja.getBoundingClientRect();
  return {
    x: clamp((e.clientX - r.left) / r.width, 0, 1),
    y: clamp((e.clientY - r.top) / r.height, 0, 1),
  };
};
const rectangulo = () => ({
  x: Math.min(ini.x, fin.x),
  y: Math.min(ini.y, fin.y),
  w: Math.abs(fin.x - ini.x),
  h: Math.abs(fin.y - ini.y),
});

function pintarSeleccion() {
  const r = rectangulo();
  const pct = (v) => `${v * 100}%`;
  sel.style.display = "block";
  sel.style.left = pct(r.x);
  sel.style.top = pct(r.y);
  sel.style.width = pct(r.w);
  sel.style.height = pct(r.h);
  sel.style.boxShadow = "0 0 0 9999px #000000a6";
  velo.style.display = "none";

  cajaTiradores.style.display = "block";
  [...cajaTiradores.children].forEach((t, i) => {
    t.style.left = pct(r.x + r.w * PUNTOS[i][0]);
    t.style.top = pct(r.y + r.h * PUNTOS[i][1]);
  });

  medida.style.display = "block";
  medida.textContent = `${Math.round(r.w * ANCHO)} × ${Math.round(r.h * ALTO)}`;
  medida.style.left = pct(r.x);
  medida.style.top =
    r.y * caja.clientHeight > 30 ? `calc(${pct(r.y)} - 26px)` : `calc(${pct(r.y + r.h)} + 8px)`;
}

function pintarLupa(p) {
  const lado = 116 / 6; // zoom 6x, el de la app
  lente.clearRect(0, 0, 116, 116);
  lente.drawImage(escritorio, p.x * ANCHO - lado / 2, p.y * ALTO - lado / 2, lado, lado, 0, 0, 116, 116);
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
  lente.strokeStyle = AZUL_APP;
  lente.lineWidth = 2;
  lente.strokeRect(55, 55, 7, 7);

  const d = pincel.getImageData(Math.round(p.x * ANCHO), Math.round(p.y * ALTO), 1, 1).data;
  hex.textContent = ("#" + [d[0], d[1], d[2]].map((v) => v.toString(16).padStart(2, "0")).join("")).toUpperCase();
  lupa.style.display = "block";
  lupa.style.left = p.x > 0.8 ? `calc(${p.x * 100}% - 136px)` : `calc(${p.x * 100}% + 20px)`;
  lupa.style.top = p.y > 0.75 ? `calc(${p.y * 100}% - 150px)` : `calc(${p.y * 100}% + 20px)`;
}

function colocarHerramientas() {
  const r = rectangulo();
  herramientas.style.display = "flex";
  const ancho = herramientas.offsetWidth;
  const cabeAbajo = (r.y + r.h) * caja.clientHeight + 56 < caja.clientHeight;
  const centro = (r.x + r.w / 2) * caja.clientWidth;
  herramientas.style.left = `${clamp(centro - ancho / 2, 8, caja.clientWidth - ancho - 8)}px`;
  herramientas.style.top = cabeAbajo
    ? `calc(${(r.y + r.h) * 100}% + 10px)`
    : `calc(${r.y * 100}% - 54px)`;
}

function limpiar() {
  hay = false;
  arrastrando = false;
  sel.style.display = "none";
  cajaTiradores.style.display = "none";
  medida.style.display = "none";
  herramientas.style.display = "none";
  velo.style.display = "block";
  aviso.style.display = "none";
}

caja.addEventListener("pointerdown", (e) => {
  if (e.target.closest(".herramientas") || e.target.closest(".modos")) return;
  if (pantallaEntera) return llevarsePantalla();
  caja.setPointerCapture(e.pointerId);
  arrastrando = true;
  hay = false;
  herramientas.style.display = "none";
  aviso.style.display = "none";
  ini = fin = enCaja(e);
  pintarSeleccion();
});

caja.addEventListener("pointermove", (e) => {
  const p = enCaja(e);
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

/* --------------------------------------------- la barra de modos de arriba */

/* Qué botones tienen sentido con lo que hay elegido: en vídeo o GIF no se copia
   una imagen, se empieza a grabar, así que sale un botón y no tres. */
function pintarModo() {
  modos.querySelectorAll("[data-modo]").forEach((b) => {
    const activo = b.dataset.modo === modo || (b.dataset.modo === "pantalla" && pantallaEntera);
    b.classList.toggle("viva", activo);
  });
  eligePantalla.classList.toggle("viva", pantallaEntera);
  cartelPantalla.textContent = frase(
    modo === "video" ? "cartelVideo" : modo === "gif" ? "cartelGif" : "cartelFoto",
  );
  const grabando = modo !== "foto";
  botonGrabar.hidden = !grabando;
  herramientas.querySelectorAll("[data-accion=copiar],[data-accion=guardar],[data-accion=editar]")
    .forEach((b) => (b.hidden = grabando));
  if (pantallaEntera) limpiar();
}

modos.addEventListener("click", (e) => {
  const boton = e.target.closest("button");
  if (!boton) return;
  if (boton.dataset.accion === "cancelar") {
    pantallaEntera = false;
    limpiar();
    return pintarModo();
  }
  if (boton.dataset.modo === "pantalla") pantallaEntera = !pantallaEntera;
  else modo = boton.dataset.modo;
  pintarModo();
});

/* Con la pantalla entera elegida no hay nada que arrastrar: un clic y ya está. */
function llevarsePantalla() {
  pantallaEntera = false;
  pintarModo();
  aviso.style.display = "flex";
  aviso.querySelector("span").textContent =
    `${frase(modo === "video" ? "pantallaVideo" : modo === "gif" ? "pantallaGif" : "pantallaFoto")} · ${ANCHO} × ${ALTO} px`;
}

caja.addEventListener("pointerleave", () => {
  if (!arrastrando && !hay) lupa.style.display = "none";
});

herramientas.addEventListener("click", (e) => {
  const boton = e.target.closest("button");
  if (!boton || boton.disabled) return;
  if (boton.dataset.accion === "cancelar") return limpiar();
  const r = rectangulo();
  const clave = boton.dataset.accion === "grabar" ? modo : boton.dataset.accion;
  aviso.style.display = "flex";
  aviso.querySelector("span").textContent =
    `${frase(clave)} · ${Math.round(r.w * ANCHO)} × ${Math.round(r.h * ALTO)} px`;
});

document.addEventListener("keydown", (e) => {
  if (e.key === "Escape" && (hay || arrastrando || pantallaEntera)) {
    pantallaEntera = false;
    pintarModo();
    return limpiar();
  }
  // Solo cuando la demo está a la vista: si no, escribir en la página movería la barra.
  if (document.getElementById("p-sel").hidden) return;
  const k = e.key.toLowerCase();
  if (k === "f" || k === "v" || k === "g") {
    modo = k === "f" ? "foto" : k === "v" ? "video" : "gif";
    pintarModo();
  } else if (k === "p") {
    pantallaEntera = !pantallaEntera;
    pintarModo();
  } else if (k === "1" && pantallaEntera) {
    llevarsePantalla();
  }
});

pintarModo();

/* ========================================================== 2. editor === */
// Calcado del editor de la 0.3.8: la grabación entera a lo ancho de la línea de tiempo, dos
// asas para quedarse con un trozo, y el Estudio con los mismos fondos que la app.
const REGION = { width: 800, height: 500 };
const FPS_GRABADO = 30;
const TOTAL = 90; // tres segundos de grabación de ejemplo
const DURACION = Math.round((TOTAL * 1000) / FPS_GRABADO);
const MINIATURAS = 10;
const MINIMO = 200; // lo menos que se puede dejar, como en la app

const lienzoEditor = document.getElementById("lienzo-editor");
lienzoEditor.width = REGION.width;
lienzoEditor.height = REGION.height;
const brocha = lienzoEditor.getContext("2d");

const pista = document.getElementById("tira-pista");
const reloj = document.getElementById("edi-reloj");
const dice = document.getElementById("tira-dice");
const totalTexto = document.getElementById("tira-total");
const entero = document.getElementById("tira-entero");
const ediSub = document.getElementById("edi-sub");
const botonPlay = document.getElementById("reproducir");

let posicion = 0;
let desde = 0;
let hasta = DURACION;
let reproduciendo = null;
let agarrado = null;

const indiceEn = (ms) => clamp(Math.floor((ms * FPS_GRABADO) / 1000), 0, TOTAL - 1);
/** «0:03.2»: minutos, segundos y décimas, como el reloj del editor. */
function tiempo(ms) {
  const d = Math.max(0, Math.floor(ms / 100));
  return `${Math.floor(d / 600)}:${String(Math.floor((d % 600) / 10)).padStart(2, "0")}.${d % 10}`;
}

/** Un fotograma de la grabación de ejemplo: el cursor llega al botón y lo pulsa. */
function fotograma(c, w, h, i) {
  const t = i / (TOTAL - 1);
  c.fillStyle = "#101018";
  c.fillRect(0, 0, w, h);
  c.fillStyle = "#181820";
  c.beginPath();
  c.roundRect(w * 0.07, h * 0.1, w * 0.86, h * 0.8, 14);
  c.fill();

  c.fillStyle = "#2a2a36";
  for (let f = 0; f < 3; f++) {
    c.beginPath();
    c.roundRect(w * 0.12, h * 0.2 + f * h * 0.11, w * (0.52 - f * 0.12), h * 0.05, 6);
    c.fill();
  }

  const pulsado = t > 0.62;
  c.fillStyle = pulsado ? AZUL_APP : "#2a5c96";
  c.beginPath();
  c.roundRect(w * 0.12, h * 0.62, w * 0.27, h * 0.14, 10);
  c.fill();
  c.fillStyle = "#fff";
  c.font = `600 ${Math.round(h * 0.06)}px ui-sans-serif, system-ui, sans-serif`;
  c.fillText(frase("exportar"), w * 0.163, h * 0.715);

  if (pulsado) {
    const onda = (t - 0.62) / 0.38;
    c.strokeStyle = `rgba(59,130,246,${1 - onda})`;
    c.lineWidth = 3;
    c.beginPath();
    c.roundRect(w * 0.12 - onda * 40, h * 0.62 - onda * 40, w * 0.27 + onda * 80, h * 0.14 + onda * 80, 16);
    c.stroke();
  }

  const avance = Math.min(1, t / 0.62);
  const suave = 1 - Math.pow(1 - avance, 3);
  const cx = w * 0.78 + (w * 0.23 - w * 0.78) * suave;
  const cy = h * 0.22 + (h * 0.69 - h * 0.22) * suave;
  c.fillStyle = "#fff";
  c.strokeStyle = "#000";
  c.lineWidth = 1.5;
  c.beginPath();
  c.moveTo(cx, cy);
  c.lineTo(cx, cy + h * 0.085);
  c.lineTo(cx + w * 0.016, cy + h * 0.062);
  c.lineTo(cx + w * 0.03, cy + h * 0.088);
  c.lineTo(cx + w * 0.039, cy + h * 0.081);
  c.lineTo(cx + w * 0.025, cy + h * 0.055);
  c.lineTo(cx + w * 0.043, cy + h * 0.052);
  c.closePath();
  c.fill();
  c.stroke();
}

/* --- la línea de tiempo, calcada de LineaDeTiempo: miniaturas por tiempo, velos, marco,
       asas y cabezal, todo en tanto por ciento para que quepa entera en cualquier ancho --- */
const minis = document.createElement("div");
minis.className = "tira-minis";
for (let k = 0; k < MINIATURAS; k++) {
  const mini = document.createElement("canvas");
  mini.width = 160;
  mini.height = 100;
  fotograma(mini.getContext("2d"), mini.width, mini.height, indiceEn(((k + 0.5) / MINIATURAS) * DURACION));
  minis.appendChild(mini);
}
const veloIzq = document.createElement("div");
const veloDer = document.createElement("div");
const marcoTira = document.createElement("div");
const asaA = document.createElement("div");
const asaB = document.createElement("div");
const cabezal = document.createElement("div");
veloIzq.className = "tira-velo izq";
veloDer.className = "tira-velo der";
marcoTira.className = "tira-marco";
asaA.className = "tira-asa a";
asaB.className = "tira-asa b";
asaA.title = frase("principio");
asaB.title = frase("final");
asaA.innerHTML = asaB.innerHTML = "<i></i>";
cabezal.className = "tira-cabezal";
pista.append(minis, veloIzq, veloDer, marcoTira, asaA, asaB, cabezal);

const pct = (ms) => `${(clamp(ms, 0, DURACION) / DURACION) * 100}%`;

function refrescarEditor() {
  fotograma(brocha, REGION.width, REGION.height, indiceEn(posicion));
  veloIzq.style.width = pct(desde);
  veloDer.style.left = pct(hasta);
  marcoTira.style.left = pct(desde);
  marcoTira.style.right = `calc(100% - ${pct(hasta)})`;
  asaA.style.left = pct(desde);
  asaB.style.left = pct(hasta);
  cabezal.style.left = pct(posicion);
  reloj.textContent = tiempo(posicion);
  const recortado = desde > 0 || hasta < DURACION;
  dice.textContent = recortado
    ? frase("teQuedasCon").replace("{trozo}", tiempo(hasta - desde)).replace("{total}", tiempo(DURACION))
    : frase("arrastra");
  totalTexto.textContent = recortado ? "" : tiempo(DURACION);
  entero.hidden = !recortado;
  ediSub.textContent = `${REGION.width} × ${REGION.height} · ${tiempo(hasta - desde)}`;
  estimar();
}

const msEn = (clientX) => {
  const r = pista.getBoundingClientRect();
  return Math.round(clamp((clientX - r.left) / Math.max(1, r.width), 0, 1) * DURACION);
};

pista.addEventListener("pointerdown", (e) => {
  if (asaA.contains(e.target)) agarrado = "desde";
  else if (asaB.contains(e.target)) agarrado = "hasta";
  else {
    agarrado = "cabezal";
    parar();
    posicion = msEn(e.clientX);
  }
  if (agarrado === "desde") asaA.classList.add("agarrada");
  if (agarrado === "hasta") asaB.classList.add("agarrada");
  pista.setPointerCapture(e.pointerId);
  refrescarEditor();
});

pista.addEventListener("pointermove", (e) => {
  if (!agarrado) return;
  const ms = msEn(e.clientX);
  // Mover un corte enseña ese sitio en la imagen, como en la app; y las asas nunca se
  // pisan: la que se mueve se para dejando lo mínimo.
  if (agarrado === "desde") {
    desde = clamp(ms, 0, hasta - MINIMO);
    posicion = desde;
  } else if (agarrado === "hasta") {
    hasta = clamp(ms, desde + MINIMO, DURACION);
    posicion = Math.max(desde, hasta - 1);
  } else {
    posicion = ms;
  }
  refrescarEditor();
});

const soltar = () => {
  agarrado = null;
  asaA.classList.remove("agarrada");
  asaB.classList.remove("agarrada");
};
pista.addEventListener("pointerup", soltar);
pista.addEventListener("pointercancel", soltar);

entero.addEventListener("click", () => {
  desde = 0;
  hasta = DURACION;
  refrescarEditor();
});

function parar() {
  if (!reproduciendo) return;
  clearInterval(reproduciendo);
  reproduciendo = null;
  botonPlay.querySelector("use").setAttribute("href", "#i-play");
  botonPlay.setAttribute("aria-label", frase("reproducir"));
}

botonPlay.addEventListener("click", () => {
  if (reproduciendo) return parar();
  // Fuera del trozo o ya en su final: se empieza por su principio.
  if (posicion < desde || posicion >= hasta - 50) posicion = desde;
  botonPlay.querySelector("use").setAttribute("href", "#i-pause");
  botonPlay.setAttribute("aria-label", frase("pausar"));
  let antes = performance.now();
  reproduciendo = setInterval(() => {
    const ahora = performance.now();
    posicion += ahora - antes;
    antes = ahora;
    // Como en la app: se para en el final del trozo, no da la vuelta.
    if (posicion >= hasta) {
      posicion = Math.max(desde, hasta - 1);
      parar();
    }
    refrescarEditor();
  }, 1000 / FPS_GRABADO);
});

/* --- el panel: el formato, el Estudio, lo demás plegado y guardar --- */
const formatos = document.getElementById("formatos");
const para = document.getElementById("edi-para");
const calidad = document.getElementById("calidad");
const fpsRango = document.getElementById("fps");
const ancho = document.getElementById("ancho");
const alto = document.getElementById("alto");
const candado = document.getElementById("candado");
const resultado = document.getElementById("edi-resultado");
const PARA = { mp4: "paraMp4", gif: "paraGif", png: "paraPng", jpg: "paraJpg" };
let formato = "mp4";
let atado = true;

function estimar() {
  const segundos = Math.max(0.1, (hasta - desde) / 1000);
  const w = Number(ancho.value) || REGION.width;
  const h = Number(alto.value) || REGION.height;
  const q = Number(calidad.value);
  const f = Number(fpsRango.value);
  let bytes;
  if (formato === "png") bytes = w * h * 3 * 0.35;
  else if (formato === "jpg") bytes = w * h * (q / 100) * 0.15;
  else if (formato === "gif") bytes = w * h * (q / 100) * 0.12 * f * segundos;
  else bytes = ((1_000_000 + (q / 100) * 11_000_000) / 8) * segundos;
  document.getElementById("v-peso").textContent = `≈ ${formatBytes(Math.round(bytes))}`;
  document.getElementById("v-calidad").textContent = `${q}%`;
  document.getElementById("v-fps").textContent = `${f} fps`;
}

formatos.addEventListener("click", (e) => {
  const b = e.target.closest("button");
  if (!b) return;
  formato = b.dataset.formato;
  [...formatos.children].forEach((otro) => otro.classList.toggle("viva", otro === b));
  // PNG y JPG sacan UN fotograma: lo que va con el tiempo no pinta nada ahí.
  const esUnaFoto = formato === "png" || formato === "jpg";
  document.querySelectorAll(".edi-solo-video").forEach((c) => (c.hidden = esUnaFoto));
  document.getElementById("fila-bucle").hidden = formato !== "gif";
  document.getElementById("btn-guardar-formato").textContent = formato.toUpperCase();
  para.textContent = frase(PARA[formato]);
  estimar();
});
para.textContent = frase(PARA[formato]);

// Los dos desplegables, como en la app: el Estudio abierto y «Más ajustes» plegado. Lo de
// dentro de uno cerrado no se puede pulsar ni alcanzar con el tabulador.
document.querySelectorAll(".edi-plegable").forEach((plegable) => {
  const boton = plegable.querySelector(".edi-plegar");
  const dentro = plegable.querySelector(".edi-dentro");
  const poner = (abierto) => {
    plegable.classList.toggle("abierto", abierto);
    boton.setAttribute("aria-expanded", String(abierto));
    dentro.inert = !abierto;
  };
  poner(plegable.classList.contains("abierto"));
  boton.addEventListener("click", () => poner(!plegable.classList.contains("abierto")));
});

// Los fondos del Estudio se ven en la imagen, como en la vista previa de la app: el fondo
// con su aire alrededor y las esquinas de la captura redondeadas.
const fondos = document.getElementById("edi-fondos");
const marcoFondo = document.getElementById("edi-fondo");
const esquinas = document.getElementById("esquinas");
const filaEsquinas = document.getElementById("fila-esquinas");
for (const b of fondos.children) if (b.dataset.fondo) b.style.setProperty("--f", b.dataset.fondo);

function ponerEsquinas() {
  document.getElementById("v-esquinas").textContent = `${esquinas.value} px`;
  lienzoEditor.style.borderRadius = marcoFondo.classList.contains("con-fondo") ? `${esquinas.value}px` : "";
}
fondos.addEventListener("click", (e) => {
  const b = e.target.closest("button");
  if (!b) return;
  [...fondos.children].forEach((otro) => otro.classList.toggle("viva", otro === b));
  const conFondo = Boolean(b.dataset.fondo);
  marcoFondo.classList.toggle("con-fondo", conFondo);
  marcoFondo.style.background = b.dataset.fondo || "";
  filaEsquinas.hidden = !conFondo;
  ponerEsquinas();
});
esquinas.addEventListener("input", ponerEsquinas);

[calidad, fpsRango].forEach((r) => r.addEventListener("input", estimar));

function ligar(origen, destino, proporcion) {
  origen.addEventListener("input", () => {
    if (!atado) return estimar();
    const v = Number(origen.value);
    if (v > 0) destino.value = Math.round(v * proporcion);
    estimar();
  });
}
ligar(ancho, alto, REGION.height / REGION.width);
ligar(alto, ancho, REGION.width / REGION.height);

candado.dataset.on = "1";
candado.addEventListener("click", () => {
  atado = !atado;
  candado.dataset.on = atado ? "1" : "0";
  candado.title = atado ? frase("bloqueada") : frase("libre");
});

document.getElementById("porcientos").addEventListener("click", (e) => {
  const b = e.target.closest("button");
  if (!b) return;
  ancho.value = Math.round((REGION.width * Number(b.dataset.pct)) / 100);
  alto.value = Math.round((REGION.height * Number(b.dataset.pct)) / 100);
  estimar();
});

[["btn-guardar", false], ["btn-copiar", true], ["btn-guardar-en", false]].forEach(([id, copia]) => {
  document.getElementById(id).addEventListener("click", () => {
    const peso = document.getElementById("v-peso").textContent.replace("≈ ", "");
    resultado.hidden = false;
    resultado.querySelector("span").textContent =
      `${frase("guardado")} · ${peso} · ${copia ? frase("copiado") + " · " : ""}${frase("abrirCarpeta")}`;
  });
});

refrescarEditor();

// Los interruptores de la maqueta son botones vacios: el dibujo lo hace el CSS, asi que
// un lector de pantalla no tiene nada que leer. Se les pone el nombre del texto que
// tienen al lado, que ya viene traducido, y se anuncian como interruptor de verdad.
function nombrarPalancas() {
  for (const p of document.querySelectorAll(".palanca")) {
    const etiqueta = p.parentElement?.querySelector(".txt");
    // textContent pega el nombre con el <small> de debajo cuando no hay espacio entre las
    // etiquetas, y sale "Lupa de pixelzoom 6x...". Se recorren los nodos y se unen con uno.
    // Hay dos formas de fila: unas llevan el texto en hijos (<span> mas <small>) y otras
    // lo llevan suelto en el propio .txt. textContent las pega sin espacio cuando no hay
    // ninguno entre las etiquetas, asi que se recorren las hojas y se unen con un punto.
    const hojas = etiqueta
      ? [...etiqueta.querySelectorAll("span, small")].filter((n) => !n.querySelector("span, small"))
      : [];
    const trozos = hojas.length ? hojas : etiqueta ? [etiqueta] : [];
    const nombre = trozos
      .map((n) => n.textContent.trim())
      .filter(Boolean)
      .join(". ")
      .replace(/\s+/g, " ");
    if (nombre) p.setAttribute("aria-label", nombre);
    p.setAttribute("role", "switch");
    p.setAttribute("aria-checked", p.dataset.on === "1" ? "true" : "false");
    if (p.disabled) p.setAttribute("aria-disabled", "true");
  }
}
nombrarPalancas();
// El candado de la proporcion tambien enciende y apaga, pero no es una .palanca, asi que
// se queda fuera del bucle de arriba y necesita su propio par role + estado.
candado.setAttribute("role", "switch");
const marcarCandado = () =>
  candado.setAttribute("aria-checked", candado.dataset.on === "1" ? "true" : "false");
marcarCandado();
// El manejador de arriba cambia data-on en su propio escuchador; con la microtarea este
// lee el valor ya actualizado en vez del anterior.
candado.addEventListener("click", () => queueMicrotask(marcarCandado));

