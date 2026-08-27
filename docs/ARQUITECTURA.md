# Arquitectura — cómo caben sesenta funciones sin que se note

> Escrito el 26 de agosto de 2026, antes de empezar la tanda T1. El plano va antes que el
> código. Las decisiones vienen de `docs/investigacion/decisiones.md`.

Hoy winshotx son 4.069 líneas de Rust y 4.487 de TypeScript, y **cabe en una cabeza**. Esa es la
ventaja que no sale en ninguna tabla comparativa, y es la que hay que proteger al meter sesenta
funciones más.

El riesgo no es el tamaño del binario: casi todo lo que entra cuesta 0 MB. El riesgo es que
**añadir la función número 40 obligue a tocar cuarenta archivos**, y que la pantalla acabe
pareciéndose a ShareX. Este documento resuelve esas dos cosas y nada más.

---

## 1. El problema, en números

Si cada función nueva se añade como se han añadido las de hoy, esto es lo que pasa:

| Archivo | Hoy | Con 60 funciones más, sin cambiar nada |
|---|---|---|
| `settings.rs` → `struct Settings` | 14 campos | ~70 campos planos |
| `commands.rs` | 482 líneas, 29 comandos | ~1.500 líneas, ~90 comandos |
| `lib.rs` → `invoke_handler!` | 29 líneas | ~90 líneas |
| `lib/ipc.ts` | 101 líneas | ~300 líneas |
| `SettingsApp.tsx` | 540 líneas | **inmanejable** |
| `SelectionCanvas.tsx` | 669 líneas | **inmanejable** |

Los cuatro primeros son molestos pero soportables. **Los dos últimos son el problema de verdad**,
y son justo los dos archivos donde vive lo que el usuario ve.

---

## 2. El registro único: `src/lib/funciones.ts`

Una entrada por capacidad. Es la **única** lista; todo lo demás se genera o se lee de aquí.

```ts
export type Funcion = {
  codigo: string;                    // "A5", el mismo de catalogo.md
  nombre: { es: string; en: string };
  donde: "overlay" | "editor" | "grabacion" | "sistema";
  /** Clave en Settings, o null si no se puede apagar (que es lo normal). */
  ajuste: keyof Settings["..."] | null;
  /** La tecla. Vale null si la función no tiene atajo propio. */
  tecla: string | null;
  /** Solo si ocupa sitio en la barra. La mayoría NO lo ocupa: ver §5. */
  icono?: LucideIcon;
};
```

**Qué se genera solo a partir de esto:**

- La pantalla de Ajustes, agrupada por `donde`, saltándose las que tienen `ajuste: null`.
- La ayuda de teclas (`?` en el overlay), que hoy no existe y con sesenta funciones hace falta.
- Los textos en los dos idiomas, sin librería de i18n: un objeto literal, cero dependencias.

**Qué NO se genera:** la implementación. Meter una función es *una entrada aquí* + *su módulo*.
Dos sitios, no cuarenta.

### La regla que mantiene el registro pequeño

`ajuste: null` es el valor normal. Está escrito en `decisiones.md` y se repite aquí porque es lo
que decide si esto funciona: **cada función tiene que funcionar bien sin configurarla; el ajuste
existe solo para apagarla.** Si una función necesita un ajuste para entenderse, está mal
diseñada. ShareX perdió a su gente por los ajustes, no por las funciones.

---

## 3. Los ajustes, por bloques y compatibles hacia atrás

`Settings` pasa de plano a anidado. **Con `#[serde(default)]` en cada bloque y en cada campo**,
porque hay gente con la 0.1.9 instalada y su `settings.json` tiene que seguir cargando:

```rust
#[derive(Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub captura: Captura,
    pub grabacion: Grabacion,
    pub estudio: Estudio,
    pub exportar: Exportar,
    pub sistema: Sistema,
}
```

Los catorce campos de hoy se reparten entre esos cinco. **Hace falta una migración**, porque un
`settings.json` viejo tiene los campos en la raíz: se lee lo viejo, se coloca en su bloque, se
guarda, y se acabó. Una función de ~30 líneas en `settings.rs` y un test que cargue un
`settings.json` de la 0.1.9 de verdad.

---

## 4. Dónde vive cada bloque

Nombres de archivo concretos, no cajas.

### B — Anotar (T5)

**Todo en el frontend. Rust no se entera.**

```
src/components/anotar/
├── Lienzo.tsx          el canvas encima del congelado
├── herramientas.ts     las seis: flecha, formas, texto, resaltador, lapiz, difuminar
└── aplanar.ts          funde anotación + congelado antes de mandar el recorte
```

`SelectionCanvas.tsx` gana **una** línea: si hay anotaciones, `aplanar()` antes de
`captureStill`. El lienzo se monta **perezosamente**, solo al pulsar una herramienta, así que
quien no anota no paga ni un milisegundo.

### F — Estudio de grabación (T8)

**Aquí está la decisión que hace que todo el bloque cueste 0 MB y 0 ms.**

```
src-tauri/src/estudio/
├── mod.rs      registra los eventos MIENTRAS se graba (solo escribe, no dibuja)
└── pintar.rs   los dibuja AL EXPORTAR: zoom, clics, teclas, cursor
src/components/editor/PanelEstudio.tsx
```

Al grabar, junto a los fotogramas QOI se escribe un `eventos.jsonl`:

```
{"ms":1240,"tipo":"clic","x":840,"y":512,"boton":"izq"}
{"ms":1980,"tipo":"tecla","texto":"Ctrl+C"}
{"ms":2010,"tipo":"cursor","x":900,"y":500}
```

Y ya está. **Nada se pinta mientras se graba.** El exportador lee ese archivo y calcula los
fotogramas clave del zoom, suaviza el cursor, dibuja el círculo del clic y pinta las teclas.

Tres cosas salen gratis de esta decisión:

1. **0 MB de instalador y 0 ms de arranque.** Es aritmética sobre fotogramas que ya están en
   disco, dentro de `exporter.rs`, que ya existe.
2. **Se puede cambiar de idea después de grabar.** Que el zoom sea más suave, o quitarlo, no
   obliga a volver a grabar. Es exactamente lo que hace Screen Studio y por lo que cobra 20 $ al
   mes.
3. **No hace falta ningún hook de teclado.**

> ### 🚫 PROHIBIDO `WH_KEYBOARD_LL`
>
> Está escrito en `docs/METAS.md`: un hook de teclado de bajo nivel **le colgó el ordenador a
> Munir**. Mientras un hook de bajo nivel no devuelve el control, Windows tiene parada la entrada
> de todo el escritorio.
>
> Se sondea con `GetAsyncKeyState` **desde el hilo que ya está capturando** a 15, 30 o 60 fps.
> La resolución del sondeo sale igual a la de los fotogramas, y eso **es exactamente lo que hace
> falta**, porque la salida son fotogramas: un clic que ocurre entre dos fotogramas no se puede
> dibujar en ninguno de los dos de todas formas.
>
> Si esa vía no da precisión suficiente, **se abandona la función**. No se vuelve al hook.

### G — Editor (T9) y H — Exportar (T2 y T9)

```
src/components/editor/          se amplía lo que ya hay
src-tauri/src/encode/jpg.rs     nuevo (H4)
src-tauri/src/encode/webp.rs    nuevo (H5)
src-tauri/src/exporter.rs       carpeta por tipo (I6) + nombre con plantilla (H10)
```

`exporter.rs` es el único sitio que decide **dónde** cae un archivo y **cómo** se llama. Hoy son
dos líneas sueltas (`exporter.rs:95` y `commands.rs`, que guarda por su cuenta): **eso se unifica
en T2**, antes de que haya cinco formatos, no después.

### Los comandos de Rust, por bloques

`commands.rs` se parte, porque 90 comandos en un archivo no se leen:

```
src-tauri/src/commands/
├── mod.rs        lo compartido y el re-export
├── captura.rs    overlay_bootstrap, capture_still, freeze_bytes…
├── grabacion.rs  start/stop/pause/cancel_recording, session_*
├── editor.rs     frame_image, export_media
└── sistema.rs    ajustes, atajos, Impr Pant, carpetas, actualizador
```

`generate_handler!` sigue necesitando la lista entera en `lib.rs`: eso no se puede evitar y
tampoco importa, porque es una lista, no lógica.

---

## 5. Cómo la pantalla no crece: **una tecla no ocupa sitio**

Esta es la regla que decide si winshotx sigue siendo winshotx.

La barra de modos de hoy, arriba y centrada, donde Windows pone la suya:

```
                    ┌─────────────────────────┐
                    │  📷   🎬   GIF   🖥      │
                    └─────────────────────────┘

        ┌ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ┐
                                                     lupa 6×
        │        selección                    │     ┌────────┐
                                                    │ #0A7FD4│
        └ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ┘     └────────┘
                       ┌──────────────────┐
                       │ 📋  💾  ✏️  📌  🔤 │   ← la barra flotante
                       └──────────────────┘
```

Con sesenta funciones, esa barra **no crece**. Las reglas:

1. **La barra de modos se queda en cuatro iconos.** Dice *qué sale* y *de qué pantalla*, y no
   dice nada más. Está cerrada.
2. **La barra flotante llega hasta cinco iconos**, y los cinco están decididos: copiar, guardar,
   editar, **anclar** (D1) y **texto** (C1, el OCR). Ni uno más. Cuando alguien quiera el sexto,
   la respuesta es no.
3. **Todo lo demás es tecla.** Temporizador `3` y `5`, repetir `R`, regla `L`, todas las
   pantallas `0`, anotar `A`… Una tecla **no ocupa sitio en pantalla**, y ahí es donde caben las
   otras cincuenta funciones.
4. **Una tecla `?`** que despliega la lista de teclas, generada del registro. Es la única
   pantalla nueva que se añade al overlay, y solo aparece si se pide.

El estudio de grabación (F) no toca el overlay en absoluto: **vive entero en el editor**, después
de grabar, que es donde la gente ya está mirando una línea de tiempo.

---

## 6. Cómo se prueba, y qué se mide en cada tanda

Las pruebas de este repo no fingen, y eso se mantiene: capturan la pantalla de verdad y exportan
GIF y MP4 que se releen para comprobar que valen.

**Tres pruebas por función:**

1. Que hace lo que dice, sobre artefactos reales.
2. **Que apagada no cuesta nada** — la que protege los 28 ms.
3. Que no rompe la captura ni la grabación de siempre.

**El frontend también se prueba, desde el 27 de agosto de 2026.** `pnpm test` levanta Vitest
sobre el MISMO Vite que compila la aplicación, con `happy-dom` haciendo de ventana y un doble de
`invoke` que contesta lo que le diga cada prueba (`src/test/preparar.ts`). Hasta entonces
`pnpm build` solo comprobaba tipos y `cargo test` no veía nada de TypeScript: todo lo que se
escribía en la interfaz salía sin red.

Lo que cubre y por qué esas cuatro cosas y no otras:

| Archivo | Qué protege |
|---|---|
| `src/lib/pantallas.test.ts` | Las coordenadas entre monitores, **con las negativas**, que es donde este proyecto se ha equivocado tres veces. |
| `src/lib/i18n.test.ts` | La mecánica de `t()` y la salud del catálogo: claves huérfanas, traducciones que se dejaron la frase en español, marcadores perdidos al traducir. |
| `src/components/**/*.test.tsx` | Que las pantallas salgan **enteras** en inglés. Es lo único que ve una frase escrita a pelo, sin pasar por `t()`. |
| `src/lib/format.test.ts` | Los contadores que se pintan mientras se graba. |

La regla que salió de montarlo: **una prueba que nunca se ha visto roja no ha probado nada.**
Las dos de idioma se estrenaron rompiendo la traducción a mano y comprobando que mordían.

**Y una que sigue faltando:** una prueba de arranque que **falle sola si pasa de 28 ms**. Hoy ese
número se mide a mano con scripts de un scratchpad, o sea que no se mide.

Al cerrar cada tanda se remiden los tres números de la línea base: milisegundos de arranque,
bytes del instalador, MB de RAM. **Ninguna cifra publicada empeora en silencio**: están en el
README, en winshotx.com y en la tarjeta social.

---

## 7. Lo que este plano NO resuelve

- **`SelectionCanvas.tsx` ya tiene 669 líneas y va a crecer.** Anotar sale a su carpeta y el
  estudio no lo toca, pero el temporizador, la forma libre, la regla y la cruceta caen todas
  ahí. **Se parte en T1**, cuando se sepa por dónde corta, no antes: partirlo ahora sería
  adivinar.
- **`emit_to` y `listen` con `target`.** Cada evento nuevo entre ventanas tiene que decidir a
  quién va. La trampa 8 de `docs/TRAMPAS.md` explica por qué esto no es un detalle: en Tauri v2
  el destino lo deciden **los dos lados**, y un oyente sin `target` recibe todo.
- **Las subidas (P2) y el búfer circular (E7)** son de la tanda T10 y tienen sus propias reglas
  escritas en `decisiones.md`. No se adelantan.
