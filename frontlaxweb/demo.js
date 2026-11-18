/* winshotx dentro de la página. Las tres pantallas están rehechas a partir del código de la
   0.4.6: mismas medidas, mismos textos, mismos atajos y los mismos iconos de lucide. El
   escritorio congelado es una foto de verdad (nuestra guía), y lo que haces en una pestaña
   sigue en la otra: lo que recortas se abre en el editor, lo que grabas sale en su línea de
   tiempo y lo que guardas aparece en el Historial de los ajustes. Sin dependencias. */

/* ------------------------------------------------- lo que comparte con la app */
const AZUL_APP = "#3b82f6";
const clamp = (v, min, max) => Math.min(max, Math.max(min, v));
const suave = (t) => (t < 0.5 ? 4 * t * t * t : 1 - Math.pow(-2 * t + 2, 3) / 2);

function formatBytes(bytes) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`;
  // La coma es la separacion decimal en espanol y el punto en ingles: se decide con el
  // idioma que declara la propia pagina, no a mano.
  const decimal = document.documentElement.lang === "en" ? "." : ",";
  return `${(bytes / (1024 * 1024)).toFixed(1).replace(".", decimal)} MB`;
}
/** «0:03.2»: minutos, segundos y décimas, como el reloj del editor. */
function tiempo(ms) {
  const d = Math.max(0, Math.floor(ms / 100));
  return `${Math.floor(d / 600)}:${String(Math.floor((d % 600) / 10)).padStart(2, "0")}.${d % 10}`;
}

/* El dial de movimiento de la casa (el script del <head>): 1, 0,25 o 0. */
const GANANCIA = parseFloat(getComputedStyle(document.documentElement).getPropertyValue("--motion-gain")) || 0;

/* Las imágenes van al lado de este archivo, y la página inglesa vive un nivel más abajo:
   se resuelven contra la dirección del propio script, no contra la de la página. */
const BASE = new URL(".", document.currentScript ? document.currentScript.src : location.href);

/* --------------------------------------------------------------- idiomas */
const FRASES = {
  pistaSel: ["Esto no es un vídeo: <b>arrastra sobre la pantalla de abajo</b>, como harías en la tuya.", "This isn't a video: <b>drag over the screen below</b>, just like you would on yours."],
  pistaFantasma: ["Así se recorta. <b>Ahora tú</b>: arrastra donde quieras, o pulsa el lápiz para editarla.", "That's how you crop. <b>Your turn</b>: drag anywhere, or press the pencil to edit it."],
  pistaHay: ["Copia, guarda, ancla o pulsa el <b>lápiz</b> para abrirla en el editor. <kbd>V</kbd> y <kbd>G</kbd> pasan a vídeo y GIF.", "Copy, save, pin, or press the <b>pencil</b> to open it in the editor. <kbd>V</kbd> and <kbd>G</kbd> switch to video and GIF."],
  pistaGrabar: ["Ahora pulsa <b>Grabar</b>, mueve el ratón por dentro y haz algún clic: saldrá en el vídeo.", "Now press <b>Record</b>, move the mouse inside and click something: it will be in the video."],
  pistaGrabando: ["Grabando: <b>mueve el ratón y haz clic</b> dentro del marco.", "Recording: <b>move the mouse and click</b> inside the frame."],
  pistaEdiVideo: ["Dale al <b>play</b>, pon un zoom en el carril de abajo, cambia el fondo o dibuja encima.", "Hit <b>play</b>, add a zoom on the lane below, change the background or draw on top."],
  pistaEdiFoto: ["Dibuja encima con las marcas de arriba, cambia el fondo y <b>guárdala</b>.", "Draw on it with the marks up top, change the background and <b>save it</b>."],
  pistaSet: ["Los ajustes de verdad, con sus cinco pestañas. Lo que guardes en la demo sale en el <b>Historial</b>.", "The real settings, with their five tabs. Whatever you save in the demo shows up in <b>History</b>."],
  teQuedasCon: ["Te quedas con {trozo} de {total}", "Keeping {trozo} of {total}"],
  arrastra: ["Arrastra los bordes azules para recortar", "Drag the blue edges to trim"],
  principio: ["Principio: arrástralo para recortar (tecla I)", "Start: drag it to trim (key I)"],
  final: ["Final: arrástralo para recortar (tecla O)", "End: drag it to trim (key O)"],
  quitarZoom: ["Quitar este zoom", "Remove this zoom"],
  reproducir: ["Reproducir", "Play"],
  pausar: ["Pausar", "Pause"],
  paraMp4: ["Vídeo con sonido. Se abre en cualquier sitio.", "Video with sound. Opens anywhere."],
  paraGif: ["Se repite solo y sin sonido. Para pegarlo en un chat.", "Loops on its own, with no sound. For pasting into a chat."],
  paraPng: ["La imagen tal cual, sin perder nada.", "The image as it is, with nothing lost."],
  paraJpg: ["La imagen, mucho más ligera.", "The image, much lighter."],
  paraPngVideo: ["Solo el fotograma que estás viendo, sin perder nada.", "Just the frame you are looking at, with nothing lost."],
  paraJpgVideo: ["Solo el fotograma que estás viendo, mucho más ligero.", "Just the frame you are looking at, much lighter."],
  guardado: ["Guardado", "Saved"],
  copiado: ["copiado", "copied"],
  abrirCarpeta: ["abrir carpeta", "open folder"],
  copiada: ["Copiada al portapapeles · {m} px. Pégala donde quieras.", "Copied to the clipboard · {m} px. Paste it anywhere."],
  noCopia: ["En la app va directa al portapapeles; desde una web, tu navegador no deja copiar imágenes.", "In the app it goes straight to the clipboard; from a web page, your browser won't copy images."],
  guardada: ["Guardada · {m} px. La tienes en tus descargas.", "Saved · {m} px. It's in your downloads."],
  guardadaApp: ["Guardada en Pictures\\winshotx · {m} px", "Saved to Pictures\\winshotx · {m} px"],
  texto: ["En la app, la T lee el texto de la imagen con el lector de Windows y te lo deja copiado.", "In the app, T reads the text in the image with Windows' own reader and copies it for you."],
  anclada: ["Anclada encima de todo. Arrástrala, o ciérrala con la X.", "Pinned on top of everything. Drag it around, or close it with the X."],
  scroll: ["Con scroll, la app baja sola por la ventana y junta toda la página en una imagen larga.", "With scrolling, the app scrolls the window by itself and stitches the whole page into one long image."],
  soltarBarra: ["Al soltar el ratón sale la barra, como ahora.", "When you release the mouse, the bar shows up, as now."],
  soltarCopia: ["Al soltar el ratón, la captura se copia sola. Lo mismo está en los ajustes.", "When you release the mouse, the capture copies itself. The same switch is in the settings."],
  pantallaFoto: ["Pantalla entera capturada · {m} px", "Whole screen captured · {m} px"],
  cartelFoto: ["o clic para capturarla entera", "or click to capture it whole"],
  cartelVideo: ["o clic para grabarla entera", "or click to record it whole"],
  cartelGif: ["o clic para grabarla entera en GIF", "or click to record it whole to GIF"],
  grabarVideo: ["Grabar vídeo", "Record video"],
  grabarGif: ["Grabar GIF", "Record GIF"],
  tapar: ["No hay correos, teléfonos, tarjetas ni DNI en esta imagen: no hay nada que tapar.", "There are no emails, phone numbers, cards or IDs in this image: nothing to cover."],
  copiarTexto: ["En la app, esto lee el texto con el lector de Windows y lo copia.", "In the app, this reads the text with Windows' own reader and copies it."],
  acelerar: ["En esta grabación no se escribe nada: esto acelera los ratos en los que tecleas.", "Nobody types in this recording: this speeds up the stretches where you type."],
  fondoArchivo: ["En la app, aquí eliges una imagen, un vídeo o un color tuyo.", "In the app, this is where you pick your own image, video or colour."],
  estilo: ["En la app, esto guarda el fondo, las esquinas y el puntero con un nombre para la próxima grabación.", "In the app, this saves the background, corners and pointer under a name for the next recording."],
  encuadrar: ["Arrastra sobre la imagen para quedarte con un trozo.", "Drag over the image to keep just a piece of it."],
  escribe: ["Escribe aquí", "Type here"],
  exportar: ["Exportar", "Export"],
  bloqueada: ["Proporción bloqueada", "Aspect ratio locked"],
  libre: ["Proporción libre", "Aspect ratio free"],
  nombreFoto: ["Captura", "Capture"],
  nombreVideo: ["Grabación", "Recording"],
};
/* Con el dedo (el móvil, la demo a pantalla completa), lo que se le pide a quien la prueba se
   dice con el dedo: no hay ratón que mover ni teclas que pulsar. */
const FRASES_DEDO = {
  pistaSel: ["Esto no es un vídeo: <b>arrastra el dedo sobre la pantalla</b>, como harías con el ratón en la tuya.", "This isn't a video: <b>drag your finger over the screen</b>, just like you would with the mouse on yours."],
  pistaHay: ["Copia, guarda, ancla o pulsa el <b>lápiz</b> para abrirla en el editor. Arriba pasas a vídeo y GIF.", "Copy, save, pin, or press the <b>pencil</b> to open it in the editor. Up top you can switch to video and GIF."],
  pistaGrabar: ["Ahora pulsa <b>Grabar</b> y arrastra el dedo por dentro, tocando algo: saldrá en el vídeo.", "Now press <b>Record</b> and drag your finger around inside, tapping something: it will be in the video."],
  pistaGrabando: ["Grabando: <b>arrastra el dedo y toca</b> dentro del marco.", "Recording: <b>drag your finger and tap</b> inside the frame."],
};
const conDedo = matchMedia("(pointer: coarse)");
/* Cada idioma tiene su propia URL, así que aquí solo hay que leer cuál es esta página. */
const idioma = document.documentElement.lang === "en" ? "en" : "es";
const frase = (clave) => ((conDedo.matches && FRASES_DEDO[clave]) || FRASES[clave])[idioma === "en" ? 1 : 0];
const conMedida = (clave, r) => frase(clave).replace("{m}", `${Math.round(r.w)} × ${Math.round(r.h)}`);

/* Dónde está cada cosa en la foto del escritorio. Lo escribe hacer-imagenes.mjs al sacar la
   foto: el recorte de la guía que sale de serie en el editor y el botón al que va el puntero. */
/* escritorio:desde */
const ESCRITORIO = {
  es: {"ancho":1600,"alto":1000,"recorte":{"x":440,"y":343,"w":557,"h":266},"boton":{"x":462,"y":532,"w":251,"h":54}},
  en: {"ancho":1600,"alto":1000,"recorte":{"x":440,"y":343,"w":557,"h":266},"boton":{"x":462,"y":532,"w":240,"h":54}},
}[idioma];
/* escritorio:hasta */

const $ = (id) => document.getElementById(id);
const pista = $("pista-demo");
const avisoEl = $("aviso");
let avisoReloj = 0;
function aviso(texto) {
  avisoEl.style.display = "flex";
  avisoEl.querySelector("span").textContent = texto;
  clearTimeout(avisoReloj);
  avisoReloj = setTimeout(() => (avisoEl.style.display = "none"), 5200);
}
function decir(clave) {
  pista.innerHTML = frase(clave);
}

/* ---------------------------------------------------------------- pestañas */
const pestanas = [...document.querySelectorAll(".pestana[data-panel]")];
let panelVisto = "p-sel";
function irA(panel) {
  pestanas.forEach((otro) => {
    const activo = otro.dataset.panel === panel;
    otro.setAttribute("aria-selected", String(activo));
    $(otro.dataset.panel).hidden = !activo;
  });
  panelVisto = panel;
  if (panel !== "p-edi") parar();
  if (panel === "p-sel") decir(hay ? "pistaHay" : "pistaSel");
  if (panel === "p-edi") decir(fuente.tipo === "foto" ? "pistaEdiFoto" : "pistaEdiVideo");
  if (panel === "p-set") decir("pistaSet");
  if (panel === "p-edi") refrescarEditor();
}
pestanas.forEach((boton) => boton.addEventListener("click", () => irA(boton.dataset.panel)));

// Las cinco pestañas de dentro de ajustes, las mismas que la ventana de verdad.
function irAjuste(id) {
  document.querySelectorAll(".pes-set button").forEach((otro) => {
    const activo = otro.dataset.set === id;
    otro.classList.toggle("viva", activo);
    $(otro.dataset.set).hidden = !activo;
  });
}
document.querySelectorAll(".pes-set button").forEach((b) => b.addEventListener("click", () => irAjuste(b.dataset.set)));

// Los segmentados y la barra de abajo: al pulsar uno, se enciende ese y se apagan sus hermanos.
document.querySelectorAll(".segmentado, .mini-seg, .his-filtros").forEach((grupo) => {
  grupo.querySelectorAll("button").forEach((boton) => {
    boton.addEventListener("click", () => {
      grupo.querySelectorAll("button").forEach((otro) => otro.classList.toggle("viva", otro === boton));
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
const ANCHO = ESCRITORIO.ancho;
const ALTO = ESCRITORIO.alto;
const escritorio = $("escritorio");
escritorio.width = ANCHO;
escritorio.height = ALTO;
const pincel = escritorio.getContext("2d", { willReadFrequently: true });
/* Si la foto viene de otro sitio (al abrir la página como archivo, sin servidor) el lienzo
   queda «manchado» y no deja leer sus píxeles: la lupa sigue, pero sin el color. */
let limpio = false;

// Mientras llega la foto, un fondo oscuro con el mismo tono.
(function () {
  const cielo = pincel.createLinearGradient(0, 0, ANCHO, ALTO);
  cielo.addColorStop(0, "#0d1420");
  cielo.addColorStop(1, "#0b0b0d");
  pincel.fillStyle = cielo;
  pincel.fillRect(0, 0, ANCHO, ALTO);
})();
const fotoEscritorio = new Image();
fotoEscritorio.decoding = "async";
fotoEscritorio.addEventListener("load", () => {
  pincel.drawImage(fotoEscritorio, 0, 0, ANCHO, ALTO);
  try {
    pincel.getImageData(0, 0, 1, 1);
    limpio = true;
  } catch {
    limpio = false;
  }
  hex.hidden = !limpio;
  pintarMiniaturas();
  refrescarEditor();
});
// La de la precarga de la página, que lleva la huella de la foto (generar-en.mjs): pedirla
// sin ella se lleva la vieja de la caché de Cloudflare, y la precarga no serviría de nada.
const precargaEscritorio = document.querySelector('link[rel="preload"][as="image"][href*="escritorio-"]');
fotoEscritorio.src = precargaEscritorio ? precargaEscritorio.href : new URL(`img/escritorio-${idioma}.webp`, BASE).href;

const caja = $("caja");
const velo = $("velo");
const sel = $("sel");
const cajaTiradores = $("tiradores");
const medida = $("medida");
const lupa = $("lupa");
const hex = $("hex");
const herramientas = $("herramientas");
const modos = $("modos");
const eligePantalla = $("elige-pantalla");
const cartelPantalla = $("cartel-pantalla");
const botonGrabar = herramientas.querySelector(".grabar");
const fantasma = $("fantasma");
const lente = $("zoom").getContext("2d");
lente.imageSmoothingEnabled = false;

/* Qué sale del recorte. Se elige arriba, antes de recortar, igual que en la app. */
let modo = "foto";
let pantallaEntera = false;
let alSoltar = "barra";

// los ocho tiradores de SelectionHandles: esquinas y puntos medios
const PUNTOS = [[0, 0], [0.5, 0], [1, 0], [0, 0.5], [1, 0.5], [0, 1], [0.5, 1], [1, 1]];
PUNTOS.forEach(() => cajaTiradores.appendChild(document.createElement("i")));

let arrastrando = false;
let hay = false;
let tocado = false; // la persona ya ha hecho algo: el fantasma no vuelve a salir
let capaAbierta = false; // la demo a pantalla completa, en el móvil (abajo, «la demo en el móvil»)
let ini = { x: 0, y: 0 };
let fin = { x: 0, y: 0 };

/** Lo encogida que está la demo ahora (1 si no lo está): la portada la encoge y la
 *  redondea al bajar. Lo que se cuenta en proporciones (de 0 a 1 sobre su caja) no lo nota;
 *  lo que convierte píxeles del ratón en píxeles de dentro tiene que dividir por esto. */
const escalaDe = (el) => el.getBoundingClientRect().width / el.offsetWidth || 1;
const enCaja = (e) => {
  const r = caja.getBoundingClientRect();
  return { x: clamp((e.clientX - r.left) / r.width, 0, 1), y: clamp((e.clientY - r.top) / r.height, 0, 1) };
};
const rectangulo = () => ({
  x: Math.min(ini.x, fin.x),
  y: Math.min(ini.y, fin.y),
  w: Math.abs(fin.x - ini.x),
  h: Math.abs(fin.y - ini.y),
});
/** El recorte en píxeles de la foto del escritorio. */
const enPixeles = (r) => ({ x: r.x * ANCHO, y: r.y * ALTO, w: r.w * ANCHO, h: r.h * ALTO });
const pct = (v) => `${v * 100}%`;

function pintarSeleccion() {
  const r = rectangulo();
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
  medida.style.top = r.y * caja.clientHeight > 30 ? `calc(${pct(r.y)} - 26px)` : `calc(${pct(r.y + r.h)} + 8px)`;
}

/** Con el dedo, la lupa va encima del dedo: a su lado quedaba debajo de la mano. */
function pintarLupa(p, dedo = false) {
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
  if (limpio) {
    const d = pincel.getImageData(Math.round(p.x * ANCHO), Math.round(p.y * ALTO), 1, 1).data;
    hex.textContent = ("#" + [d[0], d[1], d[2]].map((v) => v.toString(16).padStart(2, "0")).join("")).toUpperCase();
  }
  lupa.style.display = "block";
  if (dedo) {
    lupa.style.left = `calc(${clamp(p.x, 0.06, 0.94) * 100}% - 58px)`;
    lupa.style.top = p.y > 0.35 ? `calc(${p.y * 100}% - 200px)` : `calc(${p.y * 100}% + 70px)`;
    return;
  }
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
  herramientas.style.top = cabeAbajo ? `calc(${(r.y + r.h) * 100}% + 10px)` : `calc(${r.y * 100}% - 54px)`;
}

function limpiar() {
  hay = false;
  arrastrando = false;
  sel.style.display = "none";
  cajaTiradores.style.display = "none";
  medida.style.display = "none";
  herramientas.style.display = "none";
  velo.style.display = "block";
  if (panelVisto === "p-sel" && !grabando) decir("pistaSel");
}

/** Lo que pasa al soltar, venga del ratón o del fantasma. */
function soltarSeleccion() {
  const r = rectangulo();
  if (r.w * ANCHO < 24 || r.h * ALTO < 24) return limpiar();
  hay = true;
  lupa.style.display = "none";
  pintarSeleccion();
  if (alSoltar === "copia" && modo === "foto") {
    copiarRecorte(enPixeles(r));
    return limpiar();
  }
  colocarHerramientas();
  decir(modo === "foto" ? "pistaHay" : "pistaGrabar");
}

caja.addEventListener("pointerdown", (e) => {
  if (e.target.closest(".herramientas, .modos, .anclada, .barra-grabando")) return;
  pararFantasma();
  tocado = true;
  if (grabando) return;
  if (pantallaEntera) return llevarsePantalla();
  caja.setPointerCapture(e.pointerId);
  arrastrando = true;
  hay = false;
  herramientas.style.display = "none";
  ini = fin = enCaja(e);
  pintarSeleccion();
});
caja.addEventListener("pointermove", (e) => {
  if (e.pointerType === "mouse" && !fantasmaVivo) tocado = tocado || arrastrando;
  const p = enCaja(e);
  if (grabando) return apuntarGrabacion(p, false);
  if (arrastrando) {
    fin = p;
    pintarSeleccion();
  }
  if (!hay && !fantasmaVivo) pintarLupa(p, e.pointerType !== "mouse");
});
caja.addEventListener("pointerup", (e) => {
  if (!arrastrando) return;
  arrastrando = false;
  fin = enCaja(e);
  soltarSeleccion();
});
caja.addEventListener("pointerleave", () => {
  if (!arrastrando && !hay) lupa.style.display = "none";
});

/* --------------------------------------------- la barra de modos de arriba */
function pintarModo() {
  modos.querySelectorAll("[data-modo]").forEach((b) => {
    b.classList.toggle("viva", b.dataset.modo === modo || (b.dataset.modo === "pantalla" && pantallaEntera));
  });
  modos.querySelector("[data-accion=scroll]").hidden = modo !== "foto";
  modos.querySelector("[data-accion=soltar]").hidden = modo !== "foto";
  eligePantalla.classList.toggle("viva", pantallaEntera);
  cartelPantalla.textContent = frase(modo === "video" ? "cartelVideo" : modo === "gif" ? "cartelGif" : "cartelFoto");
  const grabandoModo = modo !== "foto";
  botonGrabar.hidden = !grabandoModo;
  $("texto-grabar").textContent = frase(modo === "gif" ? "grabarGif" : "grabarVideo");
  herramientas.querySelectorAll("[data-accion]:not(.grabar):not(.mata)").forEach((b) => (b.hidden = grabandoModo));
  if (pantallaEntera) limpiar();
  else if (hay) {
    colocarHerramientas();
    decir(modo === "foto" ? "pistaHay" : "pistaGrabar");
  }
}

function ponerAlSoltar(valor) {
  alSoltar = valor;
  modos.querySelector("[data-accion=soltar]").classList.toggle("viva", valor === "copia");
  // El mismo ajuste vive en dos sitios de la ventana de ajustes: se mantienen iguales.
  [$("seg-soltar"), $("mini-soltar")].forEach((grupo) => {
    [...grupo.querySelectorAll("button")].forEach((b, i) => b.classList.toggle("viva", i === (valor === "copia" ? 1 : 0)));
  });
}
[$("seg-soltar"), $("mini-soltar")].forEach((grupo) => {
  [...grupo.querySelectorAll("button")].forEach((b, i) => b.addEventListener("click", () => ponerAlSoltar(i ? "copia" : "barra")));
});

modos.addEventListener("click", (e) => {
  const boton = e.target.closest("button");
  if (!boton) return;
  tocado = true;
  pararFantasma();
  const accion = boton.dataset.accion;
  if (accion === "cancelar") {
    pantallaEntera = false;
    limpiar();
    return pintarModo();
  }
  if (accion === "scroll") return aviso(frase("scroll"));
  if (accion === "soltar") {
    ponerAlSoltar(alSoltar === "barra" ? "copia" : "barra");
    return aviso(frase(alSoltar === "copia" ? "soltarCopia" : "soltarBarra"));
  }
  if (accion === "ajustes") return irA("p-set");
  if (boton.dataset.modo === "pantalla") pantallaEntera = !pantallaEntera;
  else modo = boton.dataset.modo;
  pintarModo();
});

/* Con la pantalla entera elegida no hay nada que arrastrar: un clic y ya está. */
function llevarsePantalla() {
  pantallaEntera = false;
  pintarModo();
  const r = { x: 0, y: 0, w: ANCHO, h: ALTO };
  if (modo === "foto") {
    guardarEnHistorial("foto", r);
    aviso(conMedida("pantallaFoto", r));
  } else {
    ini = { x: 0, y: 0 };
    fin = { x: 1, y: 1 };
    empezarGrabacion();
  }
}

/* --------------------------------------------- la barra de la captura */
/** El recorte, pintado en un lienzo suyo. */
function lienzoDe(r) {
  const c = document.createElement("canvas");
  c.width = Math.max(1, Math.round(r.w));
  c.height = Math.max(1, Math.round(r.h));
  c.getContext("2d").drawImage(escritorio, r.x, r.y, r.w, r.h, 0, 0, c.width, c.height);
  return c;
}
function copiarLienzo(c, r) {
  if (!limpio || !navigator.clipboard || typeof ClipboardItem === "undefined") return aviso(frase("noCopia"));
  c.toBlob((blob) => {
    navigator.clipboard
      .write([new ClipboardItem({ "image/png": blob })])
      .then(() => aviso(conMedida("copiada", r)))
      .catch(() => aviso(frase("noCopia")));
  }, "image/png");
}
function descargarLienzo(c, r, nombre) {
  if (!limpio) return aviso(conMedida("guardadaApp", r));
  c.toBlob((blob) => {
    const a = document.createElement("a");
    a.href = URL.createObjectURL(blob);
    a.download = nombre;
    a.click();
    setTimeout(() => URL.revokeObjectURL(a.href), 4000);
    aviso(conMedida("guardada", r));
  }, "image/png");
}
const nombreArchivo = (ext) => {
  const d = new Date();
  const dos = (n) => String(n).padStart(2, "0");
  return `winshotx-${d.getFullYear()}-${dos(d.getMonth() + 1)}-${dos(d.getDate())}-${dos(d.getHours())}${dos(d.getMinutes())}${dos(d.getSeconds())}.${ext}`;
};
function copiarRecorte(r) {
  copiarLienzo(lienzoDe(r), r);
  guardarEnHistorial("foto", r);
}

herramientas.addEventListener("click", (e) => {
  const boton = e.target.closest("button");
  if (!boton || boton.disabled) return;
  tocado = true;
  pararFantasma();
  const accion = boton.dataset.accion;
  const r = enPixeles(rectangulo());
  if (accion === "cancelar") return limpiar();
  if (accion === "copiar") return copiarRecorte(r);
  if (accion === "guardar" || accion === "guardar-en") {
    descargarLienzo(lienzoDe(r), r, nombreArchivo("png"));
    return guardarEnHistorial("foto", r);
  }
  if (accion === "editar") {
    abrirEditor({ tipo: "foto", rect: r });
    return limpiar();
  }
  if (accion === "anclar") {
    anclar(r);
    return limpiar();
  }
  if (accion === "texto") return aviso(frase("texto"));
  if (accion === "grabar") return empezarGrabacion();
});

/* Anclar: la captura se queda flotando encima, y se mueve arrastrándola. */
function anclar(r) {
  const anclada = document.createElement("div");
  anclada.className = "anclada";
  const c = lienzoDe(r);
  const cerrar = document.createElement("button");
  cerrar.className = "anclada-x";
  cerrar.setAttribute("aria-label", idioma === "en" ? "Close" : "Cerrar");
  cerrar.innerHTML = '<svg><use href="#i-x"/></svg>';
  anclada.append(c, cerrar);
  const k = Math.min(1, 280 / r.w, 200 / r.h) * (caja.clientWidth / ANCHO) * 1.6;
  anclada.style.width = `${Math.max(90, r.w * k)}px`;
  anclada.style.left = `${(r.x / ANCHO) * 100 + 2}%`;
  anclada.style.top = `${(r.y / ALTO) * 100 + 2}%`;
  caja.appendChild(anclada);
  aviso(frase("anclada"));
  cerrar.addEventListener("click", () => anclada.remove());
  let agarre = null;
  anclada.addEventListener("pointerdown", (e) => {
    if (e.target.closest(".anclada-x")) return;
    anclada.setPointerCapture(e.pointerId);
    // El ratón va en píxeles de la pantalla, y `left`/`top` en los de dentro de la demo.
    const k = escalaDe(caja);
    agarre = { x: e.clientX / k - anclada.offsetLeft, y: e.clientY / k - anclada.offsetTop, k };
  });
  anclada.addEventListener("pointermove", (e) => {
    if (!agarre) return;
    anclada.style.left = `${clamp(e.clientX / agarre.k - agarre.x, -anclada.offsetWidth / 2, caja.clientWidth - anclada.offsetWidth / 2)}px`;
    anclada.style.top = `${clamp(e.clientY / agarre.k - agarre.y, 0, caja.clientHeight - 30)}px`;
  });
  anclada.addEventListener("pointerup", () => (agarre = null));
}

/* --------------------------------------------- grabar */
let grabando = null;
const marcoGrabar = $("marco-grabar");
const barraGrabando = $("barra-grabando");
function empezarGrabacion() {
  const r = rectangulo();
  herramientas.style.display = "none";
  cajaTiradores.style.display = "none";
  medida.style.display = "none";
  lupa.style.display = "none";
  sel.style.boxShadow = "none";
  sel.style.display = "none";
  velo.style.display = "none";
  marcoGrabar.hidden = false;
  marcoGrabar.style.left = pct(r.x);
  marcoGrabar.style.top = pct(r.y);
  marcoGrabar.style.width = pct(r.w);
  marcoGrabar.style.height = pct(r.h);
  barraGrabando.hidden = false;
  const abajo = (r.y + r.h) * caja.clientHeight + 50 < caja.clientHeight;
  barraGrabando.style.left = pct(r.x + r.w / 2);
  barraGrabando.style.top = abajo ? `calc(${pct(r.y + r.h)} + 10px)` : `calc(${pct(r.y)} + 10px)`;
  grabando = { rect: r, empezo: performance.now(), puntos: [], clics: [], formato: modo };
  decir("pistaGrabando");
  const reloj = $("reloj-grabando");
  grabando.tic = setInterval(() => {
    const s = Math.floor((performance.now() - grabando.empezo) / 1000);
    reloj.textContent = `0:${String(s).padStart(2, "0")}`;
    if (s >= 8) terminarGrabacion();
  }, 200);
}
/** Cada movimiento y cada clic dentro del marco, para que salgan en el vídeo. */
function apuntarGrabacion(p, clic) {
  const r = grabando.rect;
  const t = performance.now() - grabando.empezo;
  const x = (p.x - r.x) / r.w;
  const y = (p.y - r.y) / r.h;
  if (x < -0.05 || x > 1.05 || y < -0.05 || y > 1.05) return;
  grabando.puntos.push({ t, x: clamp(x, 0, 1), y: clamp(y, 0, 1) });
  if (clic) grabando.clics.push(t);
}
caja.addEventListener("pointerdown", (e) => {
  if (grabando && !e.target.closest(".barra-grabando")) apuntarGrabacion(enCaja(e), true);
});
function terminarGrabacion() {
  if (!grabando) return;
  clearInterval(grabando.tic);
  const g = grabando;
  grabando = null;
  marcoGrabar.hidden = true;
  barraGrabando.hidden = true;
  $("reloj-grabando").textContent = "0:00";
  velo.style.display = "block";
  hay = false;
  const duracion = clamp(performance.now() - g.empezo, 1500, 8000);
  const rect = enPixeles(g.rect);
  abrirEditor({ tipo: g.formato === "gif" ? "gif" : "video", rect, duracion, puntos: g.puntos, clics: g.clics });
}
$("parar-grabando").addEventListener("click", terminarGrabacion);

document.addEventListener("keydown", (e) => {
  if (e.target.closest && e.target.closest("input, textarea, [contenteditable]")) return;
  const k = e.key.toLowerCase();
  if (panelVisto === "p-sel") {
    // La selección que está haciendo el puntero de muestra no es de nadie: Escape lo para y la
    // quita, y a pantalla completa además cierra la capa.
    if (e.key === "Escape" && fantasmaVivo) {
      pararFantasma();
      tocado = true;
      arrastrando = false;
      return limpiar();
    }
    if (e.key === "Escape" && (hay || arrastrando || pantallaEntera || grabando)) {
      // Lo usa la demo: a pantalla completa, este Escape no cierra la capa (el siguiente sí).
      e.preventDefault();
      if (grabando) return terminarGrabacion();
      pantallaEntera = false;
      pintarModo();
      return limpiar();
    }
    if (!demoALaVista()) return;
    if (hay && modo === "foto") {
      const atajo = { enter: "copiar", e: "editar", a: "anclar", t: "texto" }[k];
      if (atajo && !e.ctrlKey && !e.metaKey && !e.altKey) {
        e.preventDefault();
        return herramientas.querySelector(`[data-accion=${atajo}]`).click();
      }
    }
    if (e.ctrlKey || e.metaKey || e.altKey) return;
    if (k === "f" || k === "v" || k === "g") {
      modo = k === "f" ? "foto" : k === "v" ? "video" : "gif";
      tocado = true;
      pararFantasma();
      pintarModo();
    } else if (k === "p") {
      pantallaEntera = !pantallaEntera;
      pintarModo();
    } else if (k === "1" && pantallaEntera) {
      llevarsePantalla();
    }
  } else if (panelVisto === "p-edi" && demoALaVista() && !e.ctrlKey && !e.metaKey && !e.altKey) {
    const teclas = { 1: "flecha", 2: "caja", 3: "texto", 4: "resaltar", 5: "paso", 6: "mosaico" };
    if (teclas[k]) elegirMarca(teclas[k]);
    if (k === "m") $("silenciar").click();
  }
});
const demoALaVista = () => {
  if (capaAbierta) return true;
  const r = $("demo").getBoundingClientRect();
  return r.top < innerHeight * 0.6 && r.bottom > innerHeight * 0.4;
};

pintarModo();

/* --------------------------------------------- el fantasma: la demo se enseña sola una vez */
/* La primera vez que se ve, un puntero hace una selección sobre la guía para que se entienda
   qué hay que hacer. En cuanto tocas algo, se aparta. Con el movimiento reducido o apagado
   no se mueve: la selección aparece hecha. */
let fantasmaVivo = false;
let fantasmaCuadro = 0;
function pararFantasma() {
  if (!fantasmaVivo) return;
  fantasmaVivo = false;
  cancelAnimationFrame(fantasmaCuadro);
  fantasma.classList.remove("visible", "pulsa");
}
function ponerFantasma(p) {
  fantasma.style.left = pct(p.x);
  fantasma.style.top = pct(p.y);
}
function lanzarFantasma() {
  if (tocado || hay || panelVisto !== "p-sel") return;
  const R = ESCRITORIO.recorte;
  const a = { x: (R.x - 6) / ANCHO, y: (R.y - 6) / ALTO };
  const b = { x: (R.x + R.w) / ANCHO, y: (R.y + R.h) / ALTO };
  if (GANANCIA < 1) {
    ini = a;
    fin = b;
    soltarSeleccion();
    decir("pistaFantasma");
    return;
  }
  fantasmaVivo = true;
  fantasma.classList.add("visible");
  const desde = { x: 0.88, y: 0.18 };
  const t0 = performance.now();
  const LLEGA = 900, ARRASTRA = 1300, ESPERA = 350;
  const paso = (ahora) => {
    if (!fantasmaVivo) return;
    const t = ahora - t0;
    if (t < LLEGA) {
      const k = suave(t / LLEGA);
      const p = { x: desde.x + (a.x - desde.x) * k, y: desde.y + (a.y - desde.y) * k };
      ponerFantasma(p);
      pintarLupa(p);
    } else if (t < LLEGA + ARRASTRA) {
      if (!arrastrando) {
        arrastrando = true;
        ini = fin = a;
        lupa.style.display = "none";
        fantasma.classList.add("pulsa");
      }
      const k = suave((t - LLEGA) / ARRASTRA);
      fin = { x: a.x + (b.x - a.x) * k, y: a.y + (b.y - a.y) * k };
      ponerFantasma(fin);
      pintarSeleccion();
    } else if (t < LLEGA + ARRASTRA + ESPERA) {
      if (arrastrando) {
        arrastrando = false;
        fin = b;
        fantasma.classList.remove("pulsa");
        soltarSeleccion();
        decir("pistaFantasma");
      }
    } else {
      // Se va hacia el lápiz de la barra y se queda ahí, señalándolo.
      const lapiz = herramientas.querySelector("[data-accion=editar]").getBoundingClientRect();
      const cr = caja.getBoundingClientRect();
      const meta = { x: (lapiz.left + lapiz.width / 2 - cr.left) / cr.width, y: (lapiz.top + lapiz.height * 0.7 - cr.top) / cr.height };
      const k = suave(clamp((t - LLEGA - ARRASTRA - ESPERA) / 700, 0, 1));
      ponerFantasma({ x: b.x + (meta.x - b.x) * k, y: b.y + (meta.y - b.y) * k });
      if (k >= 1) {
        fantasmaVivo = false;
        setTimeout(() => fantasma.classList.remove("visible"), 1600);
        return;
      }
    }
    fantasmaCuadro = requestAnimationFrame(paso);
  };
  fantasmaCuadro = requestAnimationFrame(paso);
}
new IntersectionObserver(
  (entradas, vigia) => {
    if (!entradas.some((x) => x.isIntersecting)) return;
    vigia.disconnect();
    setTimeout(lanzarFantasma, 500);
  },
  { threshold: 0.55 },
).observe(caja);

/* ========================================================== 2. editor === */
// Calcado del editor de la 0.4.6: la grabación entera a lo ancho de la línea de tiempo, dos
// asas para quedarse con un trozo, el carril de los zooms debajo, y el Estudio a la derecha.
const lienzoEditor = $("lienzo-editor");
const brocha = lienzoEditor.getContext("2d");
const pistaTira = $("tira-pista");
const reloj = $("edi-reloj");
const dice = $("tira-dice");
const totalTexto = $("tira-total");
const entero = $("tira-entero");
const ediSub = $("edi-sub");
const botonPlay = $("reproducir");
const tiempoEl = $("edi-tiempo");
const carril = $("tira-zooms");
const MINIATURAS = 10;
const MINIMO = 200; // lo menos que se puede dejar, como en la app

/** La grabación de serie: el puntero va al botón de descargar de la guía y lo pulsa. */
function fuenteDeSerie() {
  const R = ESCRITORIO.recorte;
  const B = ESCRITORIO.boton;
  const bx = (B.x + B.w / 2 - R.x) / R.w;
  const by = (B.y + B.h / 2 - R.y) / R.h;
  return { tipo: "video", rect: { ...R }, duracion: 3600, puntos: caminoHasta(bx, by, 3600), clics: [2150], zooms: [{ desde: 1650, hasta: 3350 }] };
}
/** Un puntero que llega a (x, y) con prisa al principio y despacio al final, y luego se queda. */
function caminoHasta(x, y, duracion) {
  const puntos = [];
  const desde = { x: 0.92, y: 0.12 };
  for (let t = 0; t <= duracion; t += 50) {
    const k = suave(clamp(t / 2000, 0, 1));
    const deriva = t > 2400 ? ((t - 2400) / 1200) * 0.025 : 0;
    puntos.push({ t, x: desde.x + (x - desde.x) * k + deriva, y: desde.y + (y - desde.y) * k + deriva * 0.6 });
  }
  return puntos;
}

let fuente = fuenteDeSerie();
let DURACION = fuente.duracion;
let posicion = 0;
let desde = 0;
let hasta = DURACION;
let reproduciendo = 0;
let agarrado = null;
let marcas = [];
let marcaViva = null; // la herramienta elegida arriba
let color = "#f43f5e";
let silencio = false;

function abrirEditor(nueva) {
  parar();
  const R = nueva.rect;
  if (nueva.tipo === "foto") {
    fuente = { tipo: "foto", rect: R, duracion: 0, puntos: [], clics: [], zooms: [] };
  } else {
    let puntos = nueva.puntos || [];
    let clics = nueva.clics || [];
    const duracion = nueva.duracion || 3600;
    // Si no se movió el ratón dentro, el vídeo enseña el camino de serie hacia el centro.
    if (puntos.length < 4) {
      puntos = caminoHasta(0.5, 0.55, duracion);
      clics = [Math.min(duracion - 600, 2150)];
    }
    const zooms = clics.slice(0, 3).map((c) => ({ desde: Math.max(0, c - 500), hasta: Math.min(duracion, c + 1200) }));
    fuente = { tipo: nueva.tipo, rect: R, duracion, puntos, clics, zooms: unirZooms(zooms) };
  }
  DURACION = Math.max(1, fuente.duracion);
  posicion = 0;
  desde = 0;
  hasta = DURACION;
  marcas = [];
  formato = fuente.tipo === "foto" ? "png" : fuente.tipo === "gif" ? "gif" : "mp4";
  ponerFormato(formato);
  ajustarLienzo();
  pintarMiniaturas();
  irA("p-edi");
}
function unirZooms(lista) {
  const orden = [...lista].sort((a, b) => a.desde - b.desde);
  const juntos = [];
  for (const z of orden) {
    const ultimo = juntos[juntos.length - 1];
    if (ultimo && z.desde <= ultimo.hasta) ultimo.hasta = Math.max(ultimo.hasta, z.hasta);
    else juntos.push({ ...z });
  }
  return juntos;
}

/* El lienzo mide lo que el recorte, con 1280 de ancho como mucho. */
let W = 0;
let H = 0;
function ajustarLienzo() {
  const R = fuente.rect;
  const k = Math.min(1, 1280 / R.w);
  W = Math.max(2, Math.round(R.w * k));
  H = Math.max(2, Math.round(R.h * k));
  lienzoEditor.width = W;
  lienzoEditor.height = H;
  $("ancho").value = Math.round(R.w);
  $("alto").value = Math.round(R.h);
  ediSub.textContent = "";
}

/** Dónde está el puntero en el instante t, de 0 a 1 dentro del recorte. */
function punteroEn(t) {
  const p = fuente.puntos;
  if (!p.length) return null;
  if (t <= p[0].t) return p[0];
  for (let i = 1; i < p.length; i++) {
    if (p[i].t >= t) {
      const a = p[i - 1];
      const b = p[i];
      const k = (t - a.t) / Math.max(1, b.t - a.t);
      return { x: a.x + (b.x - a.x) * k, y: a.y + (b.y - a.y) * k };
    }
  }
  return p[p.length - 1];
}
/** Con «Movimiento suave», la media de los últimos 240 ms: se le quita el temblor. */
function punteroSuave(t) {
  if (!$("p-edi").querySelector(".edi-solo-video .palanca").dataset.on === "1") return punteroEn(t);
  let x = 0;
  let y = 0;
  let n = 0;
  for (let d = 0; d <= 240; d += 40) {
    const p = punteroEn(Math.max(0, t - d));
    if (!p) return null;
    x += p.x;
    y += p.y;
    n++;
  }
  return { x: x / n, y: y / n };
}

/** Cuánto se acerca la cámara en t: los zooms del carril y, si se pide, cada clic. */
function acercamiento(t) {
  let z = 1;
  const RAMPA = 380;
  const llevar = (a, b, cuanto) => {
    if (t < a || t > b) return;
    const entra = clamp((t - a) / RAMPA, 0, 1);
    const sale = clamp((b - t) / RAMPA, 0, 1);
    z = Math.max(z, 1 + (cuanto - 1) * suave(Math.min(entra, sale)));
  };
  for (const zo of fuente.zooms) llevar(zo.desde, zo.hasta, 2);
  const porClics = Number($("zoom-clics").value) / 100;
  if (porClics > 0) for (const c of fuente.clics) llevar(c - 300, c + 1500, 1 + porClics * 1.4);
  return z;
}

function fotograma(c, w, h, t, conMarcas) {
  const R = fuente.rect;
  const esVideo = fuente.tipo !== "foto";
  const z = esVideo ? acercamiento(t) : 1;
  const p = esVideo ? punteroSuave(t) : null;
  // La cámara mira al puntero, sin salirse del recorte.
  const cx = clamp(p ? p.x : 0.5, 0.5 / z, 1 - 0.5 / z);
  const cy = clamp(p ? p.y : 0.5, 0.5 / z, 1 - 0.5 / z);
  const sx = R.x + (cx - 0.5 / z) * R.w;
  const sy = R.y + (cy - 0.5 / z) * R.h;
  c.imageSmoothingEnabled = true;
  c.imageSmoothingQuality = "high";
  c.drawImage(escritorio, sx, sy, R.w / z, R.h / z, 0, 0, w, h);
  const enLienzo = (q) => ({ x: ((R.x + q.x * R.w - sx) / (R.w / z)) * w, y: ((R.y + q.y * R.h - sy) / (R.h / z)) * h });
  if (esVideo && $("marcar-clics").dataset.on === "1") {
    for (const clic of fuente.clics) {
      const d = t - clic;
      if (d < 0 || d > 600) continue;
      const q = enLienzo(punteroEn(clic));
      const k = d / 600;
      c.strokeStyle = `rgba(59,130,246,${0.9 * (1 - k)})`;
      c.lineWidth = 3 * (w / 800);
      c.beginPath();
      c.arc(q.x, q.y, (8 + 34 * k) * (w / 800) * z, 0, Math.PI * 2);
      c.stroke();
    }
  }
  if (p) dibujarPuntero(c, enLienzo(p), (Number($("puntero").value) / 32) * (w / 900) * Math.sqrt(z));
  if (conMarcas) for (const m of marcas) dibujarMarca(c, m, w, h);
}

function dibujarPuntero(c, q, k) {
  c.save();
  c.translate(q.x, q.y);
  c.scale(k * 1.6, k * 1.6);
  c.fillStyle = "#fff";
  c.strokeStyle = "#000";
  c.lineWidth = 1.4;
  c.lineJoin = "round";
  c.beginPath();
  c.moveTo(0, 0);
  c.lineTo(0, 20);
  c.lineTo(5, 15.5);
  c.lineTo(8.6, 23);
  c.lineTo(11.6, 21.6);
  c.lineTo(8.2, 14.4);
  c.lineTo(14.6, 14);
  c.closePath();
  c.fill();
  c.stroke();
  c.restore();
}

/* --- las marcas, en coordenadas del lienzo (de 0 a 1), dibujadas encima de cada fotograma --- */
function dibujarMarca(c, m, w, h) {
  const a = { x: m.a.x * w, y: m.a.y * h };
  const b = { x: m.b.x * w, y: m.b.y * h };
  const grosor = Math.max(3, w / 220);
  c.save();
  c.strokeStyle = m.color;
  c.fillStyle = m.color;
  c.lineWidth = grosor;
  c.lineCap = "round";
  c.lineJoin = "round";
  const x = Math.min(a.x, b.x);
  const y = Math.min(a.y, b.y);
  const an = Math.abs(b.x - a.x);
  const al = Math.abs(b.y - a.y);
  if (m.tipo === "flecha" || m.tipo === "curva" || m.tipo === "linea") {
    let angulo = Math.atan2(b.y - a.y, b.x - a.x);
    c.beginPath();
    c.moveTo(a.x, a.y);
    if (m.tipo === "curva") {
      const ctrl = { x: (a.x + b.x) / 2 - (b.y - a.y) * 0.35, y: (a.y + b.y) / 2 + (b.x - a.x) * 0.35 };
      c.quadraticCurveTo(ctrl.x, ctrl.y, b.x, b.y);
      angulo = Math.atan2(b.y - ctrl.y, b.x - ctrl.x);
    } else c.lineTo(b.x, b.y);
    c.stroke();
    if (m.tipo !== "linea") {
      const punta = grosor * 4.2;
      c.beginPath();
      c.moveTo(b.x, b.y);
      c.lineTo(b.x - punta * Math.cos(angulo - 0.45), b.y - punta * Math.sin(angulo - 0.45));
      c.lineTo(b.x - punta * Math.cos(angulo + 0.45), b.y - punta * Math.sin(angulo + 0.45));
      c.closePath();
      c.fill();
    }
  } else if (m.tipo === "caja") {
    c.beginPath();
    c.roundRect(x, y, an, al, grosor * 1.4);
    c.stroke();
  } else if (m.tipo === "elipse") {
    c.beginPath();
    c.ellipse(x + an / 2, y + al / 2, an / 2, al / 2, 0, 0, Math.PI * 2);
    c.stroke();
  } else if (m.tipo === "resaltar") {
    c.globalAlpha = 0.35;
    c.fillRect(x, y, an, al);
  } else if (m.tipo === "lapiz") {
    c.beginPath();
    m.puntos.forEach((q, i) => (i ? c.lineTo(q.x * w, q.y * h) : c.moveTo(q.x * w, q.y * h)));
    c.stroke();
  } else if (m.tipo === "paso") {
    const r = Math.max(14, w / 46);
    c.beginPath();
    c.arc(a.x, a.y, r, 0, Math.PI * 2);
    c.fill();
    c.fillStyle = m.color === "#171717" || m.color === "#eab308" ? "#fff" : "#fff";
    c.font = `700 ${Math.round(r * 1.15)}px "Segoe UI", system-ui, sans-serif`;
    c.textAlign = "center";
    c.textBaseline = "middle";
    c.fillText(String(m.numero), a.x, a.y + r * 0.05);
  } else if (m.tipo === "texto") {
    const t = Math.max(18, w / 34);
    c.font = `700 ${Math.round(t)}px "Segoe UI", system-ui, sans-serif`;
    c.textBaseline = "top";
    c.lineWidth = t / 5;
    c.strokeStyle = m.color === "#171717" ? "#fff" : "#000a";
    c.strokeText(m.texto, a.x, a.y);
    c.fillText(m.texto, a.x, a.y);
  } else if (m.tipo === "mosaico") {
    // Se pinta en pequeño y se vuelve a estirar sin suavizar: los cuadros destruyen el
    // detalle, como el de la app, y no hace falta leer ni un píxel.
    if (an > 2 && al > 2) {
      const tam = Math.max(6, w / 70);
      const chico = document.createElement("canvas");
      chico.width = Math.max(1, Math.round(an / tam));
      chico.height = Math.max(1, Math.round(al / tam));
      chico.getContext("2d").drawImage(c.canvas, x, y, an, al, 0, 0, chico.width, chico.height);
      c.imageSmoothingEnabled = false;
      c.drawImage(chico, 0, 0, chico.width, chico.height, x, y, an, al);
    }
  } else if (m.tipo === "foco") {
    c.fillStyle = "rgba(0,0,0,.55)";
    c.beginPath();
    c.rect(0, 0, w, h);
    c.roundRect(x, y, an, al, grosor * 2);
    c.fill("evenodd");
  }
  c.restore();
}

const botonesMarca = [...document.querySelectorAll("[data-marca]")];
function elegirMarca(tipo) {
  marcaViva = marcaViva === tipo ? null : tipo;
  botonesMarca.forEach((b) => b.classList.toggle("viva", b.dataset.marca === marcaViva));
  lienzoEditor.classList.toggle("dibujando", Boolean(marcaViva));
}
botonesMarca.forEach((b) => b.addEventListener("click", () => elegirMarca(b.dataset.marca)));
document.querySelectorAll("#colores .color").forEach((b) => {
  b.addEventListener("click", () => {
    color = getComputedStyle(b).getPropertyValue("--c").trim();
    document.querySelectorAll("#colores .color").forEach((o) => o.classList.toggle("viva", o === b));
  });
});

const enLienzoEditor = (e) => {
  const r = lienzoEditor.getBoundingClientRect();
  return { x: clamp((e.clientX - r.left) / r.width, 0, 1), y: clamp((e.clientY - r.top) / r.height, 0, 1) };
};
let dibujando = null;
lienzoEditor.addEventListener("pointerdown", (e) => {
  if (!marcaViva) return;
  parar();
  const p = enLienzoEditor(e);
  if (marcaViva === "texto") return escribirEn(p);
  if (marcaViva === "paso") {
    marcas.push({ tipo: "paso", a: p, b: p, color, numero: marcas.filter((m) => m.tipo === "paso").length + 1 });
    return refrescarEditor();
  }
  lienzoEditor.setPointerCapture(e.pointerId);
  dibujando = { tipo: marcaViva, a: p, b: p, color, puntos: [p] };
  marcas.push(dibujando);
});
lienzoEditor.addEventListener("pointermove", (e) => {
  if (!dibujando) return;
  const p = enLienzoEditor(e);
  dibujando.b = p;
  if (dibujando.tipo === "lapiz") dibujando.puntos.push(p);
  refrescarEditor();
});
lienzoEditor.addEventListener("pointerup", () => {
  if (!dibujando) return;
  const m = dibujando;
  dibujando = null;
  const corta = Math.abs(m.a.x - m.b.x) + Math.abs(m.a.y - m.b.y) < 0.01;
  if (corta && m.tipo !== "lapiz") marcas.pop();
  refrescarEditor();
});
/* El texto se escribe encima de la imagen, en el sitio donde pinchas (0.4.6). */
function escribirEn(p) {
  const vista = $("edi-fondo");
  const campo = document.createElement("input");
  campo.className = "edi-escribe";
  campo.placeholder = frase("escribe");
  const r = lienzoEditor.getBoundingClientRect();
  const v = vista.getBoundingClientRect();
  // Medidas de la pantalla pasadas a las de dentro de la demo, que puede ir encogida.
  const k = escalaDe(vista);
  campo.style.left = `${(r.left - v.left + p.x * r.width) / k}px`;
  campo.style.top = `${(r.top - v.top + p.y * r.height) / k}px`;
  campo.style.color = color;
  vista.appendChild(campo);
  let hecho = false;
  const guardar = (vale) => {
    if (hecho) return;
    hecho = true;
    if (vale && campo.value.trim()) marcas.push({ tipo: "texto", a: p, b: p, color, texto: campo.value.trim() });
    campo.remove();
    refrescarEditor();
  };
  campo.addEventListener("keydown", (e) => {
    if (e.key === "Enter") guardar(true);
    if (e.key === "Escape") guardar(false);
  });
  // El foco, después del clic: el campo nace en el `pointerdown`, y el `mousedown` que
  // viene detrás se lleva el foco a la página, así que el campo se cerraba vacío al momento
  // y con el ratón no había forma de escribir.
  setTimeout(() => {
    campo.focus();
    campo.addEventListener("blur", () => guardar(true));
  }, 0);
}

document.querySelector(".edi-derecha").addEventListener("click", (e) => {
  const b = e.target.closest("button");
  if (!b) return;
  const accion = b.dataset.accion;
  if (accion === "deshacer") marcas.pop();
  if (accion === "quitar-todas") marcas = [];
  if (accion === "tapar") aviso(frase("tapar"));
  if (accion === "copiar-texto") aviso(frase("copiarTexto"));
  refrescarEditor();
});
document.querySelector("[data-accion=encuadrar]").addEventListener("click", () => aviso(frase("encuadrar")));

/* --- la línea de tiempo, calcada de LineaDeTiempo: miniaturas, velos, marco, asas y cabezal,
       todo en tanto por ciento para que quepa entera en cualquier ancho --- */
const minis = document.createElement("div");
minis.className = "tira-minis";
const lienzosMini = [];
for (let k = 0; k < MINIATURAS; k++) {
  const mini = document.createElement("canvas");
  mini.width = 160;
  mini.height = 100;
  lienzosMini.push(mini);
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
pistaTira.append(minis, veloIzq, veloDer, marcoTira, asaA, asaB, cabezal);

function pintarMiniaturas() {
  if (fuente.tipo === "foto") return;
  const R = fuente.rect;
  lienzosMini.forEach((mini, k) => {
    mini.height = Math.round(160 * (R.h / R.w)) || 100;
    fotograma(mini.getContext("2d"), mini.width, mini.height, ((k + 0.5) / MINIATURAS) * DURACION, false);
  });
}

const porc = (ms) => `${(clamp(ms, 0, DURACION) / DURACION) * 100}%`;
function pintarCarril() {
  carril.querySelectorAll(".tira-zoom").forEach((z) => z.remove());
  fuente.zooms.forEach((z, i) => {
    const bloque = document.createElement("button");
    bloque.className = "tira-zoom";
    bloque.style.left = porc(z.desde);
    bloque.style.width = `${((z.hasta - z.desde) / DURACION) * 100}%`;
    bloque.title = frase("quitarZoom");
    bloque.innerHTML = '<svg><use href="#i-zoom-in"/></svg> 2×';
    bloque.addEventListener("click", (e) => {
      e.stopPropagation();
      fuente.zooms.splice(i, 1);
      refrescarEditor();
    });
    carril.appendChild(bloque);
  });
  carril.classList.toggle("con-zooms", fuente.zooms.length > 0);
}
carril.addEventListener("click", (e) => {
  if (fuente.tipo === "foto") return;
  const r = carril.getBoundingClientRect();
  const t = clamp((e.clientX - r.left) / r.width, 0, 1) * DURACION;
  const largo = Math.min(1400, DURACION * 0.4);
  const a = clamp(t - largo / 2, 0, DURACION - largo);
  fuente.zooms = unirZooms([...fuente.zooms, { desde: a, hasta: a + largo }]);
  posicion = a;
  refrescarEditor();
});

function refrescarEditor() {
  if (!W) ajustarLienzo();
  const esFoto = fuente.tipo === "foto";
  tiempoEl.hidden = esFoto;
  fotograma(brocha, W, H, posicion, true);
  if (!esFoto) {
    veloIzq.style.width = porc(desde);
    veloDer.style.left = porc(hasta);
    marcoTira.style.left = porc(desde);
    marcoTira.style.right = `calc(100% - ${porc(hasta)})`;
    asaA.style.left = porc(desde);
    asaB.style.left = porc(hasta);
    cabezal.style.left = porc(posicion);
    reloj.textContent = tiempo(posicion);
    const recortado = desde > 0 || hasta < DURACION;
    dice.textContent = recortado
      ? frase("teQuedasCon").replace("{trozo}", tiempo(hasta - desde)).replace("{total}", tiempo(DURACION))
      : frase("arrastra");
    totalTexto.textContent = tiempo(hasta - desde);
    entero.hidden = !recortado;
    pintarCarril();
  }
  const R = fuente.rect;
  ediSub.textContent = esFoto ? `${Math.round(R.w)} × ${Math.round(R.h)}` : `${Math.round(R.w)} × ${Math.round(R.h)} · ${tiempo(hasta - desde)}`;
  estimar();
}

const msEn = (clientX) => {
  const r = pistaTira.getBoundingClientRect();
  return Math.round(clamp((clientX - r.left) / Math.max(1, r.width), 0, 1) * DURACION);
};
pistaTira.addEventListener("pointerdown", (e) => {
  if (asaA.contains(e.target)) agarrado = "desde";
  else if (asaB.contains(e.target)) agarrado = "hasta";
  else {
    agarrado = "cabezal";
    parar();
    posicion = msEn(e.clientX);
  }
  if (agarrado === "desde") asaA.classList.add("agarrada");
  if (agarrado === "hasta") asaB.classList.add("agarrada");
  pistaTira.setPointerCapture(e.pointerId);
  refrescarEditor();
});
pistaTira.addEventListener("pointermove", (e) => {
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
  } else posicion = ms;
  refrescarEditor();
});
const soltar = () => {
  agarrado = null;
  asaA.classList.remove("agarrada");
  asaB.classList.remove("agarrada");
};
pistaTira.addEventListener("pointerup", soltar);
pistaTira.addEventListener("pointercancel", soltar);
entero.addEventListener("click", () => {
  desde = 0;
  hasta = DURACION;
  refrescarEditor();
});

function parar() {
  if (!reproduciendo) return;
  cancelAnimationFrame(reproduciendo);
  reproduciendo = 0;
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
  const paso = (ahora) => {
    posicion += ahora - antes;
    antes = ahora;
    // Como en la app: se para en el final del trozo, no da la vuelta.
    if (posicion >= hasta) {
      posicion = Math.max(desde, hasta - 1);
      refrescarEditor();
      return parar();
    }
    refrescarEditor();
    reproduciendo = requestAnimationFrame(paso);
  };
  reproduciendo = requestAnimationFrame(paso);
});
$("silenciar").addEventListener("click", () => {
  silencio = !silencio;
  $("silenciar").querySelector("use").setAttribute("href", silencio ? "#i-volume-x" : "#i-volume-2");
});
$("tira-acelerar").addEventListener("click", () => aviso(frase("acelerar")));

/* --- el panel: el formato, el Estudio, lo demás plegado y guardar --- */
const formatos = $("formatos");
const para = $("edi-para");
const calidad = $("calidad");
const fpsRango = $("fps");
const ancho = $("ancho");
const alto = $("alto");
const candado = $("candado");
const resultado = $("edi-resultado");
let formato = "mp4";
let atado = true;

function estimar() {
  const segundos = Math.max(0.1, (hasta - desde) / 1000);
  const w = Number(ancho.value) || W;
  const h = Number(alto.value) || H;
  const q = Number(calidad.value);
  const f = Number(fpsRango.value);
  let bytes;
  if (formato === "png") bytes = w * h * 3 * 0.35;
  else if (formato === "jpg") bytes = w * h * (q / 100) * 0.15;
  else if (formato === "gif") bytes = w * h * (q / 100) * 0.12 * f * segundos;
  else bytes = ((1_000_000 + (q / 100) * 11_000_000) / 8) * segundos;
  $("v-peso").textContent = `≈ ${formatBytes(Math.round(bytes))}`;
  $("v-calidad").textContent = `${q}%`;
  $("v-fps").textContent = `${f} fps`;
}

function ponerFormato(f) {
  formato = f;
  const esFoto = fuente.tipo === "foto";
  [...formatos.children].forEach((b) => {
    b.classList.toggle("viva", b.dataset.formato === f);
    // Una foto no se guarda como vídeo: ahí solo quedan PNG y JPG, como en la app.
    b.hidden = esFoto && (b.dataset.formato === "mp4" || b.dataset.formato === "gif");
  });
  const unaFoto = f === "png" || f === "jpg";
  document.querySelectorAll(".edi-solo-video").forEach((c) => (c.hidden = unaFoto || esFoto));
  $("fila-bucle").hidden = f !== "gif";
  $("btn-guardar-formato").textContent = f.toUpperCase();
  para.textContent = frase(
    f === "mp4" ? "paraMp4" : f === "gif" ? "paraGif" : esFoto ? (f === "png" ? "paraPng" : "paraJpg") : f === "png" ? "paraPngVideo" : "paraJpgVideo",
  );
  estimar();
}
formatos.addEventListener("click", (e) => {
  const b = e.target.closest("button");
  if (b) ponerFormato(b.dataset.formato);
});

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
// con su aire alrededor, las esquinas redondeadas y la sombra.
const fondos = $("edi-fondos");
const marcoFondo = $("edi-fondo");
const esquinas = $("esquinas");
const aire = $("aire");
const sombra = $("sombra");
let fondoActual = "linear-gradient(135deg, rgb(88 116 245), rgb(168 85 217))";
for (const b of fondos.children) if (b.dataset.fondo) b.style.setProperty("--f", b.dataset.fondo);

function ponerEstudio() {
  const con = Boolean(fondoActual);
  marcoFondo.classList.toggle("con-fondo", con);
  marcoFondo.style.background = con ? fondoActual : "";
  marcoFondo.style.backgroundSize = "cover";
  // El aire se ve a escala: la vista previa enseña la imagen más pequeña de lo que sale.
  marcoFondo.style.padding = con ? `${Math.round(Number(aire.value) * 0.42)}px` : "0";
  $("v-aire").textContent = `${aire.value} px`;
  $("v-esquinas").textContent = `${esquinas.value} px`;
  lienzoEditor.style.borderRadius = con ? `${Math.round(Number(esquinas.value) * 0.6)}px` : "2px";
  lienzoEditor.style.boxShadow = con && sombra.dataset.on === "1" ? "0 10px 30px #00000073" : "none";
  for (const fila of ["fila-aire", "fila-esquinas", "fila-sombra"]) $(fila).hidden = !con;
}
fondos.addEventListener("click", (e) => {
  const b = e.target.closest("button");
  if (!b) return;
  [...fondos.children].forEach((otro) => otro.classList.toggle("viva", otro === b));
  fondoActual = b.dataset.fondo;
  ponerEstudio();
});
[esquinas, aire].forEach((r) => r.addEventListener("input", ponerEstudio));
sombra.addEventListener("click", () => queueMicrotask(ponerEstudio));
document.querySelector(".edi-dos").addEventListener("click", (e) => {
  const b = e.target.closest("button");
  if (!b) return;
  if (b.dataset.accion === "fondo-escritorio") {
    // Tu fondo de escritorio: aquí, la propia foto del escritorio de la demo.
    [...fondos.children].forEach((otro) => otro.classList.remove("viva"));
    fondoActual = `center / cover url("${fotoEscritorio.src}")`;
    return ponerEstudio();
  }
  aviso(frase("fondoArchivo"));
});
document.querySelector("[data-accion=estilo]").addEventListener("click", () => aviso(frase("estilo")));
["puntero", "zoom-clics"].forEach((id) =>
  $(id).addEventListener("input", () => {
    $("v-puntero").textContent = `${$("puntero").value} px`;
    const z = Number($("zoom-clics").value);
    const nombre = $("v-zoom-clics");
    nombre.textContent = z ? `${z}%` : idioma === "en" ? "no zoom" : "sin zoom";
    nombre.classList.toggle("apagado", !z);
    pintarMiniaturas();
    refrescarEditor();
  }),
);
$("marcar-clics").addEventListener("click", () => queueMicrotask(refrescarEditor));
[calidad, fpsRango].forEach((r) => r.addEventListener("input", estimar));

function ligar(origen, destino, eje) {
  origen.addEventListener("input", () => {
    if (!atado) return estimar();
    const v = Number(origen.value);
    const R = fuente.rect;
    if (v > 0) destino.value = Math.round(eje === "alto" ? (v * R.h) / R.w : (v * R.w) / R.h);
    estimar();
  });
}
ligar(ancho, alto, "alto");
ligar(alto, ancho, "ancho");
candado.dataset.on = "1";
candado.addEventListener("click", () => {
  atado = !atado;
  candado.dataset.on = atado ? "1" : "0";
  candado.title = atado ? frase("bloqueada") : frase("libre");
});
$("porcientos").addEventListener("click", (e) => {
  const b = e.target.closest("button");
  if (!b) return;
  ancho.value = Math.round((fuente.rect.w * Number(b.dataset.pct)) / 100);
  alto.value = Math.round((fuente.rect.h * Number(b.dataset.pct)) / 100);
  estimar();
});

/** La imagen como sale del editor: el fondo, su aire, las esquinas, la sombra y las marcas. */
function componer() {
  const pad = fondoActual ? Number(aire.value) * (W / Math.max(1, fuente.rect.w)) : 0;
  const c = document.createElement("canvas");
  c.width = Math.round(W + pad * 2);
  c.height = Math.round(H + pad * 2);
  const x = c.getContext("2d");
  if (fondoActual) {
    const colores = [...fondoActual.matchAll(/rgb\(([^)]+)\)/g)].map((m) => `rgb(${m[1]})`);
    if (fondoActual.startsWith("center")) x.drawImage(escritorio, 0, 0, c.width, c.height);
    else if (colores.length > 1) {
      const g = x.createLinearGradient(0, 0, c.width, c.height);
      g.addColorStop(0, colores[0]);
      g.addColorStop(1, colores[1]);
      x.fillStyle = g;
      x.fillRect(0, 0, c.width, c.height);
    } else {
      x.fillStyle = colores[0] || "#000";
      x.fillRect(0, 0, c.width, c.height);
    }
  }
  const radio = fondoActual ? Number(esquinas.value) * (W / Math.max(1, fuente.rect.w)) : 0;
  x.save();
  if (fondoActual && sombra.dataset.on === "1") {
    x.shadowColor = "rgba(0,0,0,.4)";
    x.shadowBlur = 30;
    x.shadowOffsetY = 10;
    x.fillStyle = "#000";
    x.beginPath();
    x.roundRect(pad, pad, W, H, radio);
    x.fill();
  }
  x.restore();
  x.save();
  x.beginPath();
  x.roundRect(pad, pad, W, H, radio);
  x.clip();
  x.drawImage(lienzoEditor, pad, pad);
  x.restore();
  return c;
}

[["btn-guardar", false], ["btn-copiar", true], ["btn-guardar-en", false]].forEach(([id, copia]) => {
  $(id).addEventListener("click", () => {
    const peso = $("v-peso").textContent.replace("≈ ", "");
    const esImagen = formato === "png" || formato === "jpg";
    if (esImagen) {
      const c = componer();
      const r = { w: c.width, h: c.height };
      if (copia) copiarLienzo(c, r);
      else descargarLienzo(c, r, nombreArchivo("png"));
      guardarEnHistorial("foto", fuente.rect, c);
    } else guardarEnHistorial("video", fuente.rect, componer(), peso);
    resultado.hidden = false;
    resultado.querySelector("span").textContent = `${frase("guardado")} · ${peso} · ${copia ? frase("copiado") + " · " : ""}${frase("abrirCarpeta")}`;
  });
});

/* =================================================== 3. ajustes: el historial === */
const historial = [];
function guardarEnHistorial(tipo, r, lienzo, peso) {
  const mini = document.createElement("canvas");
  const fuenteImg = lienzo || lienzoDe(r);
  mini.width = 320;
  mini.height = Math.max(1, Math.round((320 * fuenteImg.height) / fuenteImg.width));
  mini.getContext("2d").drawImage(fuenteImg, 0, 0, mini.width, mini.height);
  const ext = tipo === "foto" ? "png" : formato;
  historial.unshift({ tipo, mini, nombre: nombreArchivo(ext), peso: peso || formatBytes(Math.round(r.w * r.h * 3 * 0.35)) });
  pintarHistorial();
}
let filtro = "todo";
function pintarHistorial() {
  const rejilla = $("his-rejilla");
  rejilla.replaceChildren();
  const lista = historial.filter((h) => filtro === "todo" || h.tipo === filtro);
  $("his-vacio").hidden = lista.length > 0;
  for (const h of lista) {
    const fig = document.createElement("figure");
    fig.className = "his-item";
    const cap = document.createElement("figcaption");
    cap.innerHTML = `<span>${h.nombre}</span><small>${h.tipo === "foto" ? frase("nombreFoto") : frase("nombreVideo")} · ${h.peso}</small>`;
    fig.append(h.mini, cap);
    rejilla.appendChild(fig);
  }
}
$("his-filtros").addEventListener("click", (e) => {
  const b = e.target.closest("button");
  if (!b) return;
  filtro = b.dataset.filtro;
  pintarHistorial();
});

/* ------------------------------------------------------------ al arrancar */
ponerFormato("mp4");
ponerEstudio();
ajustarLienzo();
refrescarEditor();

// Los interruptores de la maqueta son botones vacios: el dibujo lo hace el CSS, asi que
// un lector de pantalla no tiene nada que leer. Se les pone el nombre del texto que
// tienen al lado, que ya viene traducido, y se anuncian como interruptor de verdad.
function nombrarPalancas() {
  for (const p of document.querySelectorAll(".palanca")) {
    const etiqueta = p.parentElement?.querySelector(".txt");
    // Hay dos formas de fila: unas llevan el texto en hijos (<span> mas <small>) y otras
    // lo llevan suelto en el propio .txt. textContent las pega sin espacio cuando no hay
    // ninguno entre las etiquetas, asi que se recorren las hojas y se unen con un punto.
    const hojas = etiqueta ? [...etiqueta.querySelectorAll("span, small")].filter((n) => !n.querySelector("span, small")) : [];
    const trozos = hojas.length ? hojas : etiqueta ? [etiqueta] : [];
    const nombre = trozos
      .map((n) => n.textContent.trim())
      .filter(Boolean)
      .join(". ")
      .replace(/\s+/g, " ");
    if (nombre) p.setAttribute("aria-label", nombre);
    p.setAttribute("role", "switch");
    p.setAttribute("aria-checked", p.dataset.on === "1" ? "true" : "false");
  }
}
nombrarPalancas();
// El candado de la proporcion tambien enciende y apaga, pero no es una .palanca.
candado.setAttribute("role", "switch");
const marcarCandado = () => candado.setAttribute("aria-checked", candado.dataset.on === "1" ? "true" : "false");
marcarCandado();
candado.addEventListener("click", () => queueMicrotask(marcarCandado));

/* ================================================== la demo en el móvil === */
/* Por debajo de 861 px la demo es la app de escritorio entera, encogida para caber (estilos.css,
   «la demo en el móvil»), como las de Orquio y Adeorq. A ese tamaño no se puede pulsar nada
   dentro: la caja entera es un botón que la abre a pantalla completa, y debajo va «Pantalla
   completa». Abierta, la caja de la demo se queda fija encima de la página (sin pedirle al
   navegador su pantalla completa, que la anuncia con un aviso encima), con «Reiniciar» y
   «Cerrar», la página de debajo quieta y sin poder tocarse, y ahí se maneja con el dedo. La
   cierran «Cerrar», Escape y Atrás. */
(function () {
  const seccion = $("demo");
  const env = seccion.querySelector(".env");
  const hueco = seccion.querySelector(".hueco-demo");
  const pantalla = $("pantalla-demo");
  const escala = pantalla.querySelector(".escala-demo");
  const botonCerrar = seccion.querySelector('[data-capa="cerrar"]');
  const movil = matchMedia("(max-width: 860px)");
  const tumbado = matchMedia("(orientation: landscape)");
  let entrada = false; // la capa puso una entrada en el historial (para que Atrás la cierre)
  let saltarVuelta = false;
  let quien = null; // lo que la abrió: al cerrar, el foco vuelve ahí
  let dormidos = [];

  /** La escala: fuera, lo que mide la caja a lo ancho; dentro, lo que quepa en su hueco. */
  function colocar() {
    if (!movil.matches) {
      escala.style.removeProperty("--demo-k");
      return;
    }
    const W = parseFloat(getComputedStyle(seccion).getPropertyValue("--demo-ancho")) || 1152;
    const H = (W * 10) / 16;
    let k;
    if (capaAbierta) {
      // Tumbado, la demo llena el hueco que le deja la fila de arriba; de pie va a lo ancho, y
      // el alto solo pone tope si con lo demás no cupiera. El marco de la caja (1 px a cada
      // lado) va aparte de la demo encogida.
      const alto = tumbado.matches ? hueco.clientHeight : env.clientHeight * 0.55;
      k = Math.max(0.05, Math.min((hueco.clientWidth - 2) / W, (alto - 2) / H));
      pantalla.style.setProperty("--caja-w", `${Math.floor(W * k) + 2}px`);
      pantalla.style.setProperty("--caja-h", `${Math.floor(H * k) + 2}px`);
    } else {
      pantalla.style.removeProperty("--caja-w");
      pantalla.style.removeProperty("--caja-h");
      k = pantalla.clientWidth / W;
    }
    escala.style.setProperty("--demo-k", String(k));
  }

  /** En el móvil y sin abrir, la caja es un botón y lo de dentro no se alcanza ni con el tabulador. */
  function ponerPapel() {
    const boton = movil.matches && !capaAbierta;
    escala.inert = boton;
    if (boton) {
      pantalla.setAttribute("role", "button");
      pantalla.tabIndex = 0;
      pantalla.setAttribute("aria-label", idioma === "en" ? "See the demo full screen" : "Ver la demo a pantalla completa");
    } else {
      pantalla.removeAttribute("role");
      pantalla.removeAttribute("tabindex");
      pantalla.removeAttribute("aria-label");
    }
  }

  function abrir(e) {
    if (capaAbierta || !movil.matches) return;
    quien = e && e.currentTarget instanceof HTMLElement ? e.currentTarget : null;
    capaAbierta = true;
    // La sección se queda con su alto: la página de debajo no se mueve ni un píxel.
    seccion.style.minHeight = `${seccion.offsetHeight}px`;
    seccion.classList.add("en-grande");
    document.documentElement.classList.add("capa-abierta");
    env.setAttribute("role", "dialog");
    env.setAttribute("aria-modal", "true");
    dormidos = [...document.body.children].filter((el) => el !== seccion && !el.inert && el.tagName !== "SCRIPT");
    dormidos.forEach((el) => (el.inert = true));
    ponerPapel();
    colocar();
    try {
      history.pushState({ capaDemo: true }, "");
      entrada = true;
    } catch {
      entrada = false;
    }
    botonCerrar.focus({ preventScroll: true });
  }

  function quitar() {
    if (!capaAbierta) return;
    capaAbierta = false;
    seccion.classList.remove("en-grande");
    document.documentElement.classList.remove("capa-abierta");
    seccion.style.removeProperty("min-height");
    env.removeAttribute("role");
    env.removeAttribute("aria-modal");
    dormidos.forEach((el) => (el.inert = false));
    dormidos = [];
    ponerPapel();
    colocar();
    if (quien && quien.isConnected) quien.focus({ preventScroll: true });
  }

  /** «Cerrar» y Escape: se cierra ya, y se gasta la entrada del historial para que Atrás no haga nada raro luego. */
  function cerrar() {
    if (!capaAbierta) return;
    const gastar = entrada && history.state && history.state.capaDemo;
    entrada = false;
    quitar();
    if (gastar) {
      saltarVuelta = true;
      history.back();
    }
  }
  addEventListener("popstate", () => {
    if (saltarVuelta) {
      saltarVuelta = false;
      return;
    }
    entrada = false;
    quitar();
  });
  document.addEventListener("keydown", (e) => {
    if (!capaAbierta || e.key !== "Escape" || e.defaultPrevented) return;
    if (e.target.closest && e.target.closest("input, textarea, [contenteditable]")) return;
    e.preventDefault();
    cerrar();
  });

  /* «Reiniciar»: la demo como al llegar, sin recargar la página. Se apunta ahora cómo está cada
     control (aún nadie ha tocado nada) y al reiniciar se pulsa lo que haga falta para volver. */
  const inicio = {
    entradas: [...seccion.querySelectorAll("input")].map((i) => [i, i.value]),
    palancas: [...seccion.querySelectorAll(".palanca")].map((p) => [p, p.dataset.on]),
    grupos: [...seccion.querySelectorAll(".segmentado, .mini-seg, .his-filtros, #edi-fondos")].map((g) => [
      g,
      [...g.querySelectorAll("button")].findIndex((b) => b.classList.contains("viva")),
    ]),
    plegables: [...seccion.querySelectorAll(".edi-plegable")].map((p) => [p, p.classList.contains("abierto")]),
    ajuste: (seccion.querySelector(".pes-set button.viva") || {}).dataset?.set,
  };
  function reiniciar() {
    pararFantasma();
    if (grabando) {
      clearInterval(grabando.tic);
      grabando = null;
      marcoGrabar.hidden = true;
      barraGrabando.hidden = true;
      $("reloj-grabando").textContent = "0:00";
    }
    caja.querySelectorAll(".anclada").forEach((a) => a.remove());
    for (const [i, v] of inicio.entradas) {
      if (i.value === v) continue;
      i.value = v;
      i.dispatchEvent(new Event("input", { bubbles: true }));
    }
    for (const [p, on] of inicio.palancas) if (p.dataset.on !== on) p.click();
    if (candado.dataset.on !== "1") candado.click();
    for (const [g, n] of inicio.grupos) {
      const b = g.querySelectorAll("button")[n];
      if (b && !b.classList.contains("viva")) b.click();
    }
    for (const [p, abierto] of inicio.plegables) if (p.classList.contains("abierto") !== abierto) p.querySelector(".edi-plegar").click();
    if (inicio.ajuste) irAjuste(inicio.ajuste);
    historial.length = 0;
    pintarHistorial();
    resultado.hidden = true;
    abrirEditor(fuenteDeSerie());
    ponerEstudio();
    modo = "foto";
    pantallaEntera = false;
    ponerAlSoltar("barra");
    irA("p-sel");
    limpiar();
    pintarModo();
    avisoEl.style.display = "none";
    // Y el puntero de la primera vez vuelve a enseñar cómo se recorta.
    tocado = false;
    setTimeout(lanzarFantasma, 400);
  }

  pantalla.addEventListener("click", (e) => {
    if (pantalla.getAttribute("role") === "button") abrir(e);
  });
  pantalla.addEventListener("keydown", (e) => {
    if (pantalla.getAttribute("role") !== "button" || (e.key !== "Enter" && e.key !== " ")) return;
    e.preventDefault();
    abrir(e);
  });
  seccion.querySelector('[data-capa="abrir"]').addEventListener("click", abrir);
  botonCerrar.addEventListener("click", cerrar);
  seccion.querySelector('[data-capa="reiniciar"]').addEventListener("click", reiniciar);
  movil.addEventListener("change", () => {
    if (!movil.matches) quitar();
    ponerPapel();
    colocar();
  });
  tumbado.addEventListener("change", colocar);
  const vigia = new ResizeObserver(colocar);
  vigia.observe(pantalla);
  vigia.observe(hueco);
  ponerPapel();
  colocar();
  // La pista de la página viene escrita para el ratón: con el dedo, se dice con el dedo.
  if (conDedo.matches && panelVisto === "p-sel" && !hay) decir("pistaSel");
})();

/* ===================================================== el resto de la portada === */
/* Capturar, grabar y editar: un paso cada 260 px de rueda mientras la sección está pegada. */
(function () {
  const pistaMomentos = $("momentos-pista");
  if (!pistaMomentos) return;
  const articulos = [...document.querySelectorAll(".momento")];
  const fotos = [...document.querySelectorAll(".momentos-fotos img")];
  const pasos = [...document.querySelectorAll("#pasos-momento button")];
  const PASO = 260;
  let actual = -1;
  const poner = (i) => {
    if (i === actual) return;
    actual = i;
    articulos.forEach((a, j) => a.classList.toggle("activo", j === i));
    fotos.forEach((f, j) => f.classList.toggle("activa", j === i));
    pasos.forEach((b, j) => (j === i ? b.setAttribute("aria-current", "step") : b.removeAttribute("aria-current")));
  };
  let pendiente = false;
  const mirar = () => {
    pendiente = false;
    if (getComputedStyle(pistaMomentos).height === "auto" || innerWidth <= 860) return poner(0);
    const recorrido = -pistaMomentos.getBoundingClientRect().top;
    poner(clamp(Math.floor((recorrido + PASO * 0.35) / PASO), 0, articulos.length - 1));
  };
  addEventListener(
    "scroll",
    () => {
      if (pendiente) return;
      pendiente = true;
      requestAnimationFrame(mirar);
    },
    { passive: true },
  );
  addEventListener("resize", mirar);

  // El vacío de arriba y de abajo de lo pegado (ver estilos.css, .momentos): se mide lo que
  // sobra entre el borde de la pantalla pegada y su contenido, y se deja solo el aire de
  // una sección normal. Se vuelve a medir si cambia el ancho, el alto o llegan las letras.
  const seccion = pistaMomentos.closest(".momentos");
  const pegado = pistaMomentos.querySelector(".momentos-pegado");
  const ajustarAire = () => {
    if (!pegado || innerWidth <= 860) {
      seccion.style.removeProperty("--momentos-sube");
      seccion.style.removeProperty("--momentos-baja");
      return;
    }
    const caja = pegado.getBoundingClientRect();
    const hijos = [...pegado.children].map((h) => h.getBoundingClientRect()).filter((r) => r.height);
    if (!hijos.length) return;
    const arriba = Math.min(...hijos.map((r) => r.top)) - caja.top;
    const abajo = caja.bottom - Math.max(...hijos.map((r) => r.bottom));
    const aire = parseFloat(getComputedStyle(document.querySelector(".seccion") || seccion).paddingTop) || 0;
    seccion.style.setProperty("--momentos-sube", `${Math.max(0, Math.round(arriba - aire))}px`);
    seccion.style.setProperty("--momentos-baja", `${Math.max(0, Math.round(abajo - aire))}px`);
  };
  new ResizeObserver(ajustarAire).observe(pegado || pistaMomentos);
  if (document.fonts) document.fonts.ready.then(ajustarAire);
  ajustarAire();
  // Los nombres de arriba llevan al paso: bajan la página justo hasta él.
  pasos.forEach((b, i) =>
    b.addEventListener("click", () => {
      const arriba = pistaMomentos.getBoundingClientRect().top + scrollY;
      scrollTo({ top: arriba + i * PASO + 2, behavior: GANANCIA ? "smooth" : "auto" });
    }),
  );
  mirar();
})();

/* Las barras de las cifras se llenan cuando se ven, cada una a su velocidad de verdad. */
(function () {
  const medidas = $("medidas");
  if (!medidas) return;
  new IntersectionObserver(
    (entradas, vigia) => {
      if (!entradas.some((x) => x.isIntersecting)) return;
      vigia.disconnect();
      medidas.classList.add("vistas");
    },
    { threshold: 0.4 },
  ).observe(medidas);
})();
