/**
 * Comprueba que la ventana de ajustes (y la bienvenida) tiene SIEMPRE el mismo tamaño (S40,
 * 15-nov-2026), y que en ese tamaño todo se ve bien.
 *
 *   pnpm build
 *   node scripts/probar-medida-ajustes.mjs
 *   node scripts/probar-medida-ajustes.mjs --fotos=<carpeta>   deja la foto de cada apartado y paso
 *
 * Munito, con la 0.5.0 y el historial lleno: «no me gusta que de repente se agrande tanto la
 * interfaz que haya que scrollear; el tamaño siempre el mismo, a no ser que lo modifiques tú».
 * De S24 a la 0.5.0 este guion comprobaba lo contrario (que la ventana medía lo que cada
 * apartado); ahora comprueba esto:
 *
 *  1. al abrir, la ventana se pone a 900 x 723 por dentro (`src/lib/ventanaFija.ts`), o al
 *     tamaño guardado si alguien la estiró, y nunca más allá de la zona útil de la pantalla;
 *  2. pasar por los ocho apartados (ida y vuelta) o por los cuatro pasos de la bienvenida no le
 *     pide a la ventana ni otro tamaño ni otro sitio;
 *  3. ningún apartado de los fijos ni ningún paso lleva rueda con la ventana a su tamaño de
 *     serie; la rueda sale solo donde no cabe (el historial lleno, una pantalla baja, una
 *     ventana encogida a mano), dentro de su columna, con el desvanecido de abajo, y la
 *     cabecera y el riel se quedan quietos;
 *  4. lo que sobra no deja una franja vacía: el dibujo de la derecha llega de arriba abajo;
 *  5. nada se sale por la derecha ni se queda cortado;
 *  6. la barra de abajo (tema, idioma, al soltar, el tour y buscar versión nueva) sale en los
 *     ajustes, entera y sin letra de menos de 12 px, y no sale en la bienvenida.
 *
 * Casos: 100, 125 y 150 % (a 150 %, en una pantalla de 1080, Grabar no cabe entero); español,
 * inglés y chino; el anillo encendido; estrechada a mano a 780 (lo mínimo de `tauri.conf.json`);
 * estirada a mano a 1100 x 800; un portátil de 1366 x 768 al 125 % (566 px lógicos útiles);
 * y el historial con 60 capturas.
 *
 * Usa Edge sin ventana (el motor de WebView2) sobre `ver-ventana.mjs --servir`, cuyo Tauri de
 * mentira apunta lo que pide la página (`setSize`, `setPosition`). Sale con 0 si todo cuadra
 * y con 1 si algo no.
 */
import { spawn } from "node:child_process";
import { mkdirSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";

const RAIZ = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const fotos = process.argv.find((a) => a.startsWith("--fotos="))?.slice(8) ?? null;
if (fotos) mkdirSync(fotos, { recursive: true });

/** Lo que mide la barra de título de mentira de `ver-ventana.mjs`. */
const MARCO = 32;
/** Los apartados del riel, en su orden (`src/components/settings/mapaDeAjustes.ts`). */
const NOMBRES = ["capturar", "despues", "grabar", "ultimos", "historial", "teclas", "app", "ayuda"];
/** El tamaño de serie (`ANCHO_VENTANA` y `ALTO_VENTANA` de `src/lib/ventanaFija.ts`). */
const SERIE = { ancho: 900, alto: 723 };
/** El aire de abajo de la columna de las filas (`pb-7`): el dibujo llega hasta ahí. */
const AIRE_ABAJO = 28;

const CASOS = [
  { nombre: "100 %", escala: 1, zona: 1032, idioma: "es" },
  { nombre: "125 %", escala: 1.25, zona: 816, idioma: "es" },
  { nombre: "150 %", escala: 1.5, zona: 672, idioma: "es" },
  { nombre: "inglés", escala: 1, zona: 1032, idioma: "en" },
  { nombre: "chino", escala: 1, zona: 1032, idioma: "zh" },
  { nombre: "anillo encendido", escala: 1, zona: 1032, idioma: "es", anillo: true },
  { nombre: "estrechada a mano a 780", escala: 1, zona: 1032, idioma: "es", guardada: { ancho: 780, alto: 723 } },
  { nombre: "estrechada a 780, en inglés", escala: 1, zona: 1032, idioma: "en", guardada: { ancho: 780, alto: 723 } },
  { nombre: "estrechada a 780, en chino", escala: 1, zona: 1032, idioma: "zh", guardada: { ancho: 780, alto: 723 } },
  { nombre: "estirada a mano a 1100 x 800", escala: 1, zona: 1032, idioma: "es", guardada: { ancho: 1100, alto: 800 } },
  { nombre: "portátil 1366 x 768 al 125 %", escala: 1.25, zona: 566, idioma: "es" },
  { nombre: "historial con 60 capturas", escala: 1, zona: 1032, idioma: "es", historial: 60 },
  { nombre: "bienvenida", escala: 1, zona: 1032, idioma: "es", bienvenida: true },
  { nombre: "bienvenida en inglés", escala: 1, zona: 1032, idioma: "en", bienvenida: true },
  { nombre: "bienvenida en chino", escala: 1, zona: 1032, idioma: "zh", bienvenida: true },
  { nombre: "bienvenida a 125 %", escala: 1.25, zona: 816, idioma: "es", bienvenida: true },
  { nombre: "bienvenida en el portátil", escala: 1.25, zona: 566, idioma: "es", bienvenida: true },
];

let mal = 0;
const navegador = await chromium.launch({ channel: "msedge", headless: true });

for (const caso of CASOS) {
  console.log(`\n${caso.nombre}`);
  const quiere = caso.guardada ?? SERIE;
  const servidor = spawn(
    "node",
    [
      join(RAIZ, "scripts", "ver-ventana.mjs"),
      "--servir",
      `--zona=${caso.zona}`,
      `--idioma=${caso.idioma}`,
      ...(caso.anillo ? ["--anillo"] : []),
      ...(caso.bienvenida ? ["--bienvenida"] : []),
      ...(caso.historial ? [`--historial=${caso.historial}`] : []),
      ...(caso.guardada ? [`--guardada=${caso.guardada.ancho},${caso.guardada.alto}`] : []),
    ],
    { cwd: RAIZ },
  );
  const url = await new Promise((listo) => servidor.stdout.on("data", (d) => listo(String(d).trim())));
  // La ventana nace como dice `tauri.conf.json`: 900 x 723.
  const contexto = await navegador.newContext({
    viewport: { width: SERIE.ancho, height: SERIE.alto },
    deviceScaleFactor: caso.escala,
    reducedMotion: "reduce",
  });
  const pagina = await contexto.newPage();
  const errores = [];
  pagina.on("pageerror", (e) => errores.push(String(e)));
  await pagina.goto(url);
  await pagina.waitForSelector(caso.bienvenida ? "[data-scroll-bienvenida]" : "[data-contenido-ajustes]");
  await pagina.waitForTimeout(600);

  /** Lo que la página le ha pedido a su ventana: tamaños y sitios, en orden. */
  const pedidos = () =>
    pagina.evaluate(() =>
      (window.__llamadas ?? [])
        .filter((l) => l.cmd === "plugin:window|set_size" || l.cmd === "plugin:window|set_position")
        .map((l) => {
          // `setSize(new PhysicalSize(…))` llega envuelto en un `Size` (o `Position`) de Tauri.
          const v = l.args?.value;
          const s = v?.size ?? v?.position ?? v ?? {};
          return { cmd: l.cmd.split("|")[1], width: s.width, height: s.height, x: s.x, y: s.y };
        }),
    );

  // 1. Al abrir: lo que pide (o nada, si ya nace a su tamaño), y la ventana a eso.
  const alAbrir = await pedidos();
  const tamanno = alAbrir.filter((p) => p.cmd === "set_size").at(-1);
  const ancho = tamanno ? Math.round(tamanno.width / caso.escala) : SERIE.ancho;
  const alto = tamanno ? Math.ceil(tamanno.height / caso.escala - 0.01) : SERIE.alto;
  // Lo más que cabe por dentro en esa pantalla, en px lógicos.
  const tope = Math.floor((caso.zona * caso.escala - Math.round(MARCO * caso.escala)) / caso.escala);
  const altoEsperado = Math.min(quiere.alto, tope);
  const apretada = altoEsperado < quiere.alto;
  const abreBien = ancho === quiere.ancho && Math.abs(alto - altoEsperado) <= 1;
  if (!abreBien) mal += 1;
  console.log(
    `  ${abreBien ? "BIEN" : "MAL "} abre a ${ancho} x ${alto}${apretada ? ` (no cabe ${quiere.alto}: la zona útil deja ${tope})` : ""}${
      abreBien ? "" : `, y tenía que abrir a ${quiere.ancho} x ${altoEsperado}`
    }`,
  );
  await pagina.setViewportSize({ width: ancho, height: alto });
  await pagina.waitForTimeout(300);
  const pedidosAlAbrir = (await pedidos()).length;

  // La barra de abajo (vuelve en S40): en los ajustes sí, entera (nada cortado ni fuera) y sin
  // letra de menos de 12; en la bienvenida, no. Se mira al abrir y después de pulsar «Buscar
  // actualizaciones», que cambia el botón por la respuesta, más larga.
  const mirarBarra = async (cuando) => {
    const barra = await pagina.evaluate(() => {
      const b = document.querySelector("[data-barra-abajo]");
      if (!b) return null;
      const caja = b.getBoundingClientRect();
      const piezas = [...b.querySelectorAll("button, span")].filter((el) => el.getBoundingClientRect().width > 0);
      return {
        alto: Math.round(caja.height),
        cabe:
          b.scrollWidth <= b.clientWidth + 1 &&
          piezas.every((el) => el.getBoundingClientRect().right <= innerWidth + 0.5 && el.scrollWidth <= el.clientWidth + 1),
        letraMinima: Math.min(...piezas.map((el) => parseFloat(getComputedStyle(el).fontSize))),
        abajo: Math.round(innerHeight - caja.bottom),
        texto: b.textContent.trim().replace(/\s+/g, " ").slice(-40),
      };
    });
    const bien = caso.bienvenida ? barra === null : !!barra && barra.cabe && barra.letraMinima >= 12 && barra.abajo === 0;
    if (!bien) mal += 1;
    console.log(
      `  ${bien ? "BIEN" : "MAL "} ${
        caso.bienvenida
          ? `sin la barra de abajo${barra ? ": SALE" : ""}`
          : barra
            ? `la barra de abajo ${cuando}: ${barra.alto} px, ${barra.cabe ? "cabe entera" : "NO CABE"}, letra de ${barra.letraMinima} px como poco («…${barra.texto}»)`
            : "NO HAY BARRA DE ABAJO"
      }`,
    );
  };
  await mirarBarra("al abrir");
  if (!caso.bienvenida) {
    await pagina.locator("[data-barra-abajo] button").last().click();
    await pagina.waitForTimeout(400);
    await mirarBarra("tras buscar versión nueva");
  }

  const medir = (selHueco, selDibujo) =>
    pagina.evaluate(
      ({ selHueco, selDibujo }) => {
        const h = document.querySelector(selHueco);
        const caja = h.getBoundingClientRect();
        const dibujo = selDibujo ? document.querySelector(selDibujo) : null;
        const d = dibujo?.getBoundingClientRect();
        // Lo que se sale por la derecha o lo que el navegador ha tenido que cortar.
        const fuera = [...document.querySelectorAll("body *")]
          .filter((el) => {
            if (![...el.childNodes].some((n) => n.nodeType === 3 && n.textContent.trim())) return false;
            const r = el.getBoundingClientRect();
            return r.width > 0 && (r.right > innerWidth + 0.5 || el.scrollWidth > el.clientWidth + 1);
          })
          .map((el) => el.textContent.trim().slice(0, 40));
        // Lo que queda vacío debajo de lo último de la columna de las filas (solo para contarlo).
        const filas = document.querySelector("[data-contenido-ajustes] main, [data-paso] > div > div:first-child");
        const ultimo = filas ? [...filas.querySelectorAll("*")].reduce((m, el) => Math.max(m, el.getBoundingClientRect().bottom), 0) : 0;
        return {
          rueda: Math.round((h.scrollHeight - h.clientHeight) * 10) / 10,
          desvanecido: !!document.querySelector(".bg-gradient-to-t.from-lienzo"),
          // Del borde de abajo del dibujo al del hueco: con el aire de la columna, 28.
          bajoDibujo: d ? Math.round((caja.bottom - d.bottom) * 10) / 10 : null,
          vacio: ultimo ? Math.round(caja.bottom - ultimo) : null,
          fuera,
        };
      },
      { selHueco, selDibujo },
    );

  /** Lo que se mira en cada apartado o paso; `fijo` es que tiene que caber sin rueda. */
  const mirar = async (nombre, selHueco, selDibujo, fijo) => {
    const m = await medir(selHueco, selDibujo);
    const nuevos = (await pedidos()).length - pedidosAlAbrir;
    let bien = nuevos === 0 && m.fuera.length === 0;
    // Sin rueda con el tamaño de serie; si alguien la ha encogido a mano (o la pantalla no da
    // para más), lo que no cabe baja con la rueda, que es lo que pidió Munito.
    if (fijo && !apretada && !caso.guardada) bien &&= m.rueda <= 0;
    if (!caso.bienvenida) bien &&= m.rueda > 0 ? m.desvanecido : !m.desvanecido;
    // Cabiendo, el dibujo llega hasta abajo: nada de una franja vacía debajo de todo.
    if (m.bajoDibujo !== null && m.rueda <= 0) bien &&= Math.abs(m.bajoDibujo - AIRE_ABAJO) <= 1;
    if (!bien) mal += 1;
    console.log(
      `  ${bien ? "BIEN" : "MAL "} ${nombre.padEnd(9)} rueda ${String(m.rueda).padStart(5)}${
        m.bajoDibujo !== null ? `  bajo el dibujo ${m.bajoDibujo}` : ""
      }${m.vacio !== null ? `  vacío bajo las filas ${m.vacio}` : ""}${m.desvanecido ? "  desvanecido" : ""}${
        nuevos ? `  PIDIÓ ${nuevos} CAMBIOS DE VENTANA` : ""
      }${m.fuera.length ? `  CORTADO: ${m.fuera.join(" | ")}` : ""}`,
    );
    return m;
  };

  const foto = async (nombre) => {
    if (fotos && (caso.idioma === "es" || caso.bienvenida)) {
      await pagina.screenshot({ path: join(fotos, `${caso.nombre.replace(/[^\w]+/g, "-")}-${nombre}.png`) });
    }
  };

  if (caso.bienvenida) {
    // 2. Los cuatro pasos, con el botón de la esquina, como una persona.
    for (let paso = 0; paso < 4; paso++) {
      if (paso > 0) {
        await pagina.locator("footer button").last().click();
        await pagina.waitForTimeout(450);
      }
      await mirar(`paso ${paso + 1}`, "[data-scroll-bienvenida]", "[data-paso] figure", true);
      await foto(`paso-${paso + 1}`);
    }
  } else {
    // 2. Los ocho apartados, en orden y luego al revés.
    for (const i of [0, 1, 2, 3, 4, 5, 6, 7, 6, 3, 1, 0]) {
      const nombre = NOMBRES[i];
      await pagina.locator(`[data-riel] [data-vista="${nombre}"]`).click();
      await pagina.waitForTimeout(250);
      const enHistorial = nombre === "historial";
      const m = await mirar(nombre, "[data-scroll-ajustes]", "[data-dibujo-de]", !enHistorial);
      await foto(nombre);
      if (enHistorial && caso.historial) {
        // 3. El historial lleno baja DENTRO: la cabecera y el riel no se mueven.
        const antes = await pagina.evaluate(() => [
          document.querySelector("header, [data-cabecera]")?.getBoundingClientRect().top ?? null,
          document.querySelector("[data-riel]").getBoundingClientRect().top,
        ]);
        await pagina.evaluate(() => {
          const h = document.querySelector("[data-scroll-ajustes]");
          h.scrollTop = h.scrollHeight;
        });
        await pagina.waitForTimeout(250);
        const despues = await pagina.evaluate(() => [
          document.querySelector("header, [data-cabecera]")?.getBoundingClientRect().top ?? null,
          document.querySelector("[data-riel]").getBoundingClientRect().top,
          document.scrollingElement.scrollTop,
        ]);
        const quietas = antes[0] === despues[0] && antes[1] === despues[1] && despues[2] === 0;
        const bien = m.rueda > 0 && quietas;
        if (!bien) mal += 1;
        console.log(
          `  ${bien ? "BIEN" : "MAL "} el historial lleno baja dentro (${m.rueda} px) con la cabecera y el riel quietos${
            quietas ? "" : ` (se movieron: ${JSON.stringify(antes)} → ${JSON.stringify(despues)})`
          }`,
        );
        await foto("historial-abajo");
        await pagina.evaluate(() => document.querySelector("[data-scroll-ajustes]").scrollTo(0, 0));
      }
    }
  }
  if (errores.length) {
    mal += 1;
    console.log("  MAL  errores de la página:", errores);
  }
  await contexto.close();
  servidor.kill();
}

await navegador.close();
console.log(mal ? `\n${mal} MAL` : "\ntodo BIEN");
process.exit(mal ? 1 : 0);
