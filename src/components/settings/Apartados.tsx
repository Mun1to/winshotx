/**
 * Las filas de cada apartado de los ajustes, un componente por apartado (S36).
 *
 * Son las filas de siempre, con toda su lógica: se han movido desde `SettingsApp.tsx`, no se
 * han reescrito. Lo que cambia es dónde van (ocho apartados en vez de cinco pestañas) y lo
 * que dicen debajo del nombre: la explicación ya la cuenta el dibujo de la derecha, así que
 * solo quedan los datos (la carpeta, lo que ocupa la caché, de dónde graba el anillo) y los
 * avisos. La explicación larga sigue en el icono de cada fila.
 *
 * Cada fila lleva su `id`, el mismo que tiene en el buscador (`mapaDeAjustes.ts`): así el
 * buscador sabe a qué apartado ir y qué fila iluminar.
 */
import { useState } from "react";
import type { Update } from "@tauri-apps/plugin-updater";
import {
  AppWindow,
  Bell,
  BookOpen,
  Camera,
  Clipboard,
  Coffee,
  Compass,
  Crop,
  Download,
  EyeOff,
  FolderOpen,
  Gauge,
  Github,
  HardDrive,
  History,
  Info,
  Keyboard,
  Languages,
  Mic,
  Monitor,
  MousePointer2,
  Palette,
  Power,
  Scissors,
  SquarePen,
  Star,
  Timer,
  TriangleAlert,
  Video,
  Volume2,
  ZoomIn,
} from "lucide-react";
import { CAFE, FALLOS, REPO, WEB, comoSeLee } from "../../lib/enlaces";
import { formatBytes } from "../../lib/format";
import { aplicarIdioma, useT } from "../../lib/i18n";
import {
  openFolder,
  openUrl,
  openWindowsApps,
  elegirEditorExterno,
  configurarRecortesDeWindows,
  pickDirectory,
  replaySave,
  showScreenNumber,
  type CamaraConectada,
} from "../../lib/ipc";
import { aplicarTema } from "../../lib/tema";
import type {
  CacheStats,
  CaptureFlow,
  Language,
  PrintScreenState,
  ReplayStatus,
  Screen,
  Settings,
  ShortcutStatus,
  SonidoDisponible,
  TeclasDelEscritorio,
  Theme,
} from "../../lib/types";
import { Segmented } from "../ui/Segmented";
import { Switch } from "../ui/Switch";
import { BotonActualizar, type Fase } from "./Actualizacion";
import { FilaTeclaZoom, FilasCamara } from "./AjustesGrabacion";
import { FilaMiniatura, FondoEnLasCapturas } from "./DespuesDeCapturar";
import { OtrosGrabadores } from "./OtrosGrabadores";
import { Grupo, Row, RowButton } from "./Section";
import { CAPTURA_WINDOWS, GRABACION_WINDOWS, partesDeAtajo } from "../../lib/teclas";
import { TeclasNativas } from "./TeclasNativas";
import { ShortcutField } from "./ShortcutField";
import { OrdenDelAtajo, TeclaPuesta } from "./TeclaDelEscritorio";

/** Todo lo que necesitan las filas: los ajustes, lo que cuenta Rust de la máquina y las acciones. */
export interface DatosAjustes {
  /** S21 (Linux): lo que solo es de Windows se esconde. */
  enLinux: boolean;
  /** S21 (Linux): en Wayland las teclas no las pone winshotx, las pone el escritorio. */
  enWayland: boolean;
  /** Las teclas que ha puesto el escritorio en Wayland (null mientras contesta). */
  teclasDelEscritorio: TeclasDelEscritorio | null;
  /** Donde el escritorio no deja pedir teclas (Sway) o «Cambiar» no abre nada (Hyprland). */
  teclasAMano: boolean;
  /** La orden que lanza winshotx, para ponerla a mano en los atajos del escritorio. */
  ordenDeLaApp: string;
  cambiarTeclas: () => void;
  /** S45: el «Cambiar» de la tecla de rescatar, que el escritorio vuelve a preguntar si se rechazó. */
  cambiarTeclaDeRescate: () => void;
  /** S44 (Linux): qué sonido se puede grabar aquí. null en Windows y mientras contesta. */
  sonido: SonidoDisponible | null;
  /** S45: al capturar ya suena el obturador del escritorio (GNOME en Wayland), y el nuestro calla. */
  obturadorDelEscritorio: boolean;
  settings: Settings;
  patch: (partial: Partial<Settings>) => void;
  shortcuts: ShortcutStatus;
  replay: ReplayStatus;
  pantallas: Screen[];
  cache: CacheStats;
  vaciarCache: () => void;
  imprPant: PrintScreenState | null;
  cambiarImprPant: (quiere: boolean) => void;
  /** null = sin tocar · "confirmar" = esperando el segundo clic · el resto, el resultado. */
  recortes: string | null;
  quitarRecortes: () => void;
  /** Mientras se reinicia el Explorador, que son un par de segundos largos. */
  aplicando: boolean;
  aplicarTecla: () => void;
  cambiarWinShiftS: (quiere: boolean) => void;
  modernaOk: boolean;
  camaras: CamaraConectada[];
  /** A qué tamaño va a grabar el anillo con lo que hay elegido. */
  medidaGrabada: { ancho: number; alto: number };
  onVerBienvenida: () => void;
  onTour: () => void;
  version: string;
  actualizacion: {
    fase: Fase;
    texto: string;
    instalar: (update: Update) => void;
    mirar: () => void;
  };
}

const FPS_OPTIONS = [
  { value: 15, label: "15 fps" },
  { value: 30, label: "30 fps" },
  { value: 60, label: "60 fps" },
];

/**
 * Cuanto guarda hacia atras el anillo.
 *
 * Tres valores y no una casilla donde escribir un numero: aqui no se elige un numero, se
 * elige entre «lo que acaba de pasar», «la jugada entera» y «el minuto malo».
 */
const SEGUNDOS_ATRAS = [
  { value: 15, label: "15 s" },
  { value: 30, label: "30 s" },
  { value: 60, label: "60 s" },
];

/**
 * A cuántos fotogramas graba el anillo.
 *
 * Es el ajuste que de verdad decide lo que cuesta tenerlo puesto, porque aquí se graba todo
 * el rato: lo que vale un fotograma se paga toda la tarde.
 */
const FPS_ANILLO = [
  { value: 15, label: "15" },
  { value: 30, label: "30" },
  { value: 60, label: "60" },
];

/**
 * A qué tamaño guarda el anillo.
 *
 * Es el ajuste que más disco ahorra de los tres: bajar una pantalla de 1080p a 720p deja
 * el fotograma en menos de la mitad, y para «ver qué acaba de pasar» 720p se ve
 * perfectamente. El cero es el tamaño de la pantalla, sin tocar nada.
 */
const ALTO_ANILLO = [
  { value: 720, label: "720p" },
  { value: 1080, label: "1080p" },
  { value: 0, label: "Nativa" },
];

/**
 * Lo que puede llegar a ocupar el anillo, en bytes.
 *
 * Sale de lo mismo que el tope de Rust (`buffer::bytes_max`), y tiene que dar el mismo
 * número: 2,3 MB por cada dos millones de píxeles, que es lo que mide un fotograma cuando
 * la pantalla cambia entera, medido sobre una partida a 1920 × 1080. Un escritorio de
 * trabajo gasta una décima parte, así que esto es el techo y no lo normal.
 *
 * Cuenta con la resolución además de con los fotogramas: grabar a 720p desde una pantalla
 * de 1080p ocupa menos de la mitad, y decir lo contrario sería asustar de más.
 */
const PEOR_POR_PIXEL = 2_300_000 / (1920 * 1080);
const TECHO = 4 * 1024 * 1024 * 1024;
export const loQuePuedeOcupar = (segundos: number, fps: number, ancho: number, alto: number) =>
  Math.min(ancho * alto * PEOR_POR_PIXEL * fps * segundos, TECHO);

/**
 * Lo que cuesta el anillo, dicho de las dos combinaciones que se midieron de verdad.
 *
 * Medido el 31 de agosto de 2026 en un equipo con tres pantallas: a 60 fotogramas y
 * resolución nativa se llevaba el 86% de un núcleo, y a 30 con 1080 el 57%. El 17 de
 * septiembre, después de reciclar los búferes y de guardar solo las zonas que cambian, el 22%
 * y el 17%. Y en noviembre S29 lo rehizo comparando en la tarjeta gráfica (el −86 % de la 0.5).
 *
 * **Desde la 0.5 va sin cifra** (S36): los números de septiembre ya no son los de ahora, y
 * los nuevos se miden en el PC de Munito con lo de S29 antes de ponerlos aquí. Mientras, la
 * fila dice lo que sí es seguro: cuál es la más fluida y que gasta más con la pantalla en
 * movimiento, y en ámbar, porque nadie elige «60, nativa» sabiendo lo que cuesta: se elige
 * porque suena a mejor. Munir lo tuvo así semanas y notó el ordenador con lag.
 *
 * **Y solo se dicen esas dos**, que son las que se midieron: de las otras siete se diría algo
 * que nadie ha comprobado.
 */
export const LO_QUE_COSTO: Record<string, string> = {
  "60|0": "la más fluida; gasta más cuanto más se mueve la pantalla",
  "30|1080": "gasta poco con la pantalla quieta",
};

/**
 * S45 (Linux): por qué está apagado el anillo cuando no lo apagó la persona (`ReplayStatus`).
 *
 * En Wayland no se enciende solo al arrancar si el escritorio no tiene guardado el permiso de
 * compartir la pantalla, y se apaga si la persona deja de compartirla desde el aviso del
 * escritorio. Un interruptor que aparece apagado sin decir nada parece un fallo.
 */
const APAGADO_POR: Record<string, string> = {
  "sin-permiso": "se apagó al arrancar: falta tu permiso para compartir la pantalla",
  cortado: "se apagó porque se dejó de compartir la pantalla",
  fallo: "se apagó porque la pantalla dejó de llegar",
};

const FLUJOS: { value: CaptureFlow; label: string }[] = [
  { value: "toolbar", label: "Sale la barra" },
  { value: "instant", label: "Se copia sola" },
];

/** El automatico va primero porque es el de fabrica y el que acierta casi siempre. */
const TEMAS: { value: Theme; label: string }[] = [
  { value: "sistema", label: "Automático" },
  { value: "claro", label: "Claro" },
  { value: "oscuro", label: "Oscuro" },
];

/** Los idiomas que habla hoy. Anadir uno mas es una linea aqui y un archivo mas. */
const IDIOMAS: { value: Language; label: string }[] = [
  { value: "sistema", label: "Automático" },
  { value: "es", label: "Español" },
  { value: "en", label: "Inglés" },
  // El nombre de cada idioma va en ESE idioma, que es como lo busca quien lo necesita:
  // alguien que abre la aplicacion en chino no esta buscando la palabra «Chino».
  { value: "zh", label: "中文" },
];

/**
 * Tres opciones y ninguna más: un campo de números aquí solo sirve para escribir 47. En corto
 * («No», «3 s», «5 s»), para que quepan al lado del nombre en la columna de en medio.
 */
const ESPERAS = [
  { value: 0, label: "No" },
  { value: 3, label: "3 s" },
  { value: 5, label: "5 s" },
];

/**
 * S21 (Linux): la línea de debajo de un atajo, y su tono. En Wayland no se habla de
 * combinaciones ocupadas: la tecla la pone el escritorio, o se pone a mano donde no deja pedirla.
 */
function usePistaDeAtajo({ teclasAMano, enWayland }: Pick<DatosAjustes, "teclasAMano" | "enWayland">) {
  const t = useT();
  const pista = (libre: boolean, puesta: boolean) =>
    teclasAMano
      ? t("ponla en los atajos del escritorio")
      : enWayland
        ? puesta
          ? t("la tecla la elige el escritorio")
          : t("todavía sin tecla: pulsa «Cambiar»")
        : libre
          ? undefined
          : t("esa combinación está ocupada");
  const tono = (puesta: boolean): "dato" | "aviso" => (teclasAMano || (enWayland && puesta) ? "dato" : "aviso");
  return { pista, tono };
}

/* ------------------------------------------------------------------ Capturar */

export function ApartadoCapturar({
  imprPant,
  settings,
  patch,
  shortcuts,
  modernaOk,
  enLinux,
  enWayland,
  teclasDelEscritorio,
  teclasAMano,
  ordenDeLaApp,
  cambiarTeclas,
}: DatosAjustes) {
  const t = useT();
  // «3 s» y «5 s» se escriben igual en los tres idiomas: solo se traduce el «No».
  const esperas = ESPERAS.map((o) => (o.value === 0 ? { ...o, label: t(o.label) } : o));
  const { pista: pistaDeAtajo, tono: tonoDeAtajo } = usePistaDeAtajo({ teclasAMano, enWayland });
  return (
    <>
      <Grupo title={t("Los atajos")} tour="al-pulsar">
        <Row
          id="capRegion"
          icon={<Camera className="size-4" />}
          label={t("Capturar región")}
          explicacion={
            teclasAMano
              ? t("Congela la pantalla en el momento de pulsarla y te deja recortar encima de esa foto quieta. Este escritorio no deja que las apps pidan teclas: pon esta orden en sus atajos de teclado (en Sway, una línea bindsym en ~/.config/sway/config).")
              : enWayland
                ? t("Congela la pantalla en el momento de pulsarla y te deja recortar encima de esa foto quieta. En Wayland la tecla la elige el escritorio: winshotx le propone una, y la aceptas o la cambias en su ventana con «Cambiar».")
                : enLinux
                  ? t("Congela la pantalla en el momento de pulsarla y te deja recortar encima de esa foto quieta: lo que se mueva debajo ya no te estorba. El escritorio se reserva algunas combinaciones y esas no llegan hasta aquí.")
                  : t("Congela la pantalla en el momento de pulsarla y te deja recortar encima de esa foto quieta: lo que se mueva debajo ya no te estorba. Windows se reserva algunas combinaciones para él y esas no llegan hasta aquí.")
          }
          hint={imprPant?.screenclip ? (imprPant.porEncargo ? t("Atajo de Windows") : partesDeAtajo(CAPTURA_WINDOWS).join(" + ")) : pistaDeAtajo(shortcuts.capture, Boolean(teclasDelEscritorio?.capture))}
          tone={tonoDeAtajo(Boolean(teclasDelEscritorio?.capture))}
          control={
            imprPant?.screenclip && imprPant.porEncargo ? (
              <strong>{partesDeAtajo(CAPTURA_WINDOWS).join(" + ")}</strong>
            ) : teclasAMano ? (
              <OrdenDelAtajo orden={ordenDeLaApp} accion="capture" />
            ) : enWayland ? (
              <span className="flex items-center gap-1.5">
                <TeclaPuesta tecla={teclasDelEscritorio?.capture ?? null} />
                <RowButton onClick={cambiarTeclas}>{t("Cambiar")}</RowButton>
              </span>
            ) : (
              <ShortcutField
                value={settings.captureShortcut}
                active={shortcuts.capture}
                onChange={(v) => patch({ captureShortcut: v })}
              />
            )
          }
        />
        <Row
          id="capEditor"
          icon={<SquarePen className="size-4" />}
          label={t("Capturar y abrir el editor")}
          explicacion={t(
            "La misma captura, pero al soltar el ratón se abre el editor en vez de la barra de siempre. Para la captura que hay que trabajar: señalar algo con una flecha, tapar un dato o recortar antes de mandarla.",
          )}
          hint={imprPant?.porEncargo ? t("Disponible en el modo de bandeja") : pistaDeAtajo(shortcuts.edit, Boolean(teclasDelEscritorio?.edit))}
          tone={tonoDeAtajo(Boolean(teclasDelEscritorio?.edit))}
          control={
            teclasAMano ? (
              <OrdenDelAtajo orden={ordenDeLaApp} accion="edit" />
            ) : enWayland ? (
              <span className="flex items-center gap-1.5">
                <TeclaPuesta tecla={teclasDelEscritorio?.edit ?? null} />
                <RowButton onClick={cambiarTeclas}>{t("Cambiar")}</RowButton>
              </span>
            ) : (
              <ShortcutField
                value={settings.editShortcut}
                active={Boolean(imprPant?.porEncargo) || shortcuts.edit}
                onChange={(v) => patch({ editShortcut: v })}
              />
            )
          }
        />
      </Grupo>
      <Grupo title={t("Al capturar")}>
        <Row
          id="capEsperar"
          icon={<Timer className="size-4" />}
          label={t("Esperar antes de capturar")}
          explicacion={t("La pantalla se congela justo al pulsar el atajo, así que un menú abierto se cierra antes de que dé tiempo a fotografiarlo. Con 3 o 5 segundos, pulsas primero, abres el menú y la foto se toma sola cuando ya está delante.")}
          control={
            <Segmented
              etiqueta={t("Esperar antes de capturar")}
              value={settings.captureDelaySeconds}
              options={esperas}
              onChange={(v) => patch({ captureDelaySeconds: v })}
            />
          }
        />
        {/* Los iconos los esconde el Explorador de Windows; en Linux no hace nada. */}
        {!enLinux && (
          <Row
            id="capIconos"
            icon={<EyeOff className="size-4" />}
            label={t("Ocultar iconos del escritorio")}
            explicacion={t("Los iconos se esconden solo durante el disparo, no mientras eliges el recorte, y vuelven aunque la captura falle. Sirve para fotografiar el fondo de pantalla o una ventana con el escritorio detrás sin que salgan tus archivos.")}
            control={
              <Switch
                checked={settings.hideDesktopIcons}
                onChange={(v) => patch({ hideDesktopIcons: v })}
                label={t("Ocultar iconos del escritorio")}
              />
            }
          />
        )}
        <Row
          id="capCursor"
          icon={<MousePointer2 className="size-4" />}
          label={t("Incluir el cursor")}
          explicacion={t("Si el puntero del ratón sale dentro de la captura. Para señalar un botón viene bien; para una captura limpia sobra, y hay que acordarse antes de disparar porque después ya no se puede quitar. Solo afecta a las fotos: en las grabaciones el puntero nunca va cocido, se anota por dónde va y el editor lo dibuja al exportar con su misma imagen, del tamaño que quieras.")}
          control={
            <Switch
              checked={settings.captureCursor}
              onChange={(v) => patch({ captureCursor: v })}
              label={t("Incluir el cursor")}
            />
          }
        />
        <Row
          id="capLupa"
          icon={<ZoomIn className="size-4" />}
          label={t("Lupa de píxel")}
          explicacion={t("Mientras arrastras, una lupa de 6 aumentos con retícula te enseña el píxel exacto que hay bajo el cursor y su color en hexadecimal. Con la tecla C te llevas ese color al portapapeles sin capturar nada.")}
          control={
            <Switch
              checked={settings.showMagnifier}
              onChange={(v) => patch({ showMagnifier: v })}
              label={t("Lupa de píxel")}
            />
          }
        />
        {/* Solo cuando pasa, que es casi nunca: en un Windows al dia no sale. No es un
            ajuste y no se puede tocar nada, pero sin esto las fotos salen con trozos en
            negro y parece culpa de la app. */}
        {!modernaOk && !enLinux && (
          <Row
            icon={<TriangleAlert className="size-4" />}
            label={t("Este Windows usa la captura antigua")}
            explicacion={t(
              "winshotx hace las fotos con la captura moderna de Windows, que sí ve lo que dibuja la tarjeta gráfica. Esta versión de Windows no la admite, así que van por la de siempre y una ventana acelerada puede salir con trozos en negro. No hay nada que puedas tocar, y las grabaciones no se ven afectadas.",
            )}
            hint={t("Las fotos de ventanas con aceleración pueden salir con trozos en negro")}
            tone="aviso"
            control={null}
          />
        )}
      </Grupo>
    </>
  );
}

/* ------------------------------------------------------------------ Después de capturar */

export function ApartadoDespues({ settings, patch, obturadorDelEscritorio }: DatosAjustes) {
  const t = useT();
  const flujos = FLUJOS.map((o) => ({ ...o, label: t(o.label) }));
  return (
    <Grupo tour="despues-de-capturar">
      <Row
        id="desSoltar"
        icon={<Crop className="size-4" />}
        label={t("Al soltar el ratón")}
        explicacion={t("Con la barra sales a decidir: copiar, guardar, editar, anclar encima de todo o leer el texto. Sin ella el recorte se va directo al portapapeles y el overlay desaparece, que es un clic menos cuando siempre haces lo mismo.")}
        stacked
        control={
          <Segmented
            etiqueta={t("Al soltar el ratón")}
            value={settings.captureFlow}
            options={flujos}
            onChange={(v) => patch({ captureFlow: v })}
          />
        }
      />
      <Row
        id="desCopiar"
        icon={<Clipboard className="size-4" />}
        label={t("Copiar al guardar")}
        explicacion={t("Guardar deja el archivo en tu carpeta, y con esto además lo deja pegable. Es para cuando quieres las dos cosas a la vez: el archivo para luego y la imagen para pegarla ahora mismo en un chat.")}
        control={
          <Switch
            checked={settings.copyAfterCapture}
            onChange={(v) => patch({ copyAfterCapture: v })}
            label={t("Copiar al guardar")}
          />
        }
      />
      <FilaMiniatura settings={settings} patch={patch} />
      <Row
        id="desSonido"
        icon={<Volume2 className="size-4" />}
        label={t("Sonido de obturador")}
        explicacion={t("Un clic de cámara al capturar, como el de una réflex. Sirve para saber que la captura ha salido sin mirar la pantalla. Viene apagado: en una reunión o grabando un tutorial, un sonido que no esperas es peor que ninguno.")}
        // S45: en GNOME con Wayland el clic de la captura lo pone el escritorio, se quiera o no.
        hint={obturadorDelEscritorio ? t("al capturar ya suena el de GNOME") : undefined}
        control={
          <Switch
            checked={settings.playSound}
            onChange={(v) => patch({ playSound: v })}
            label={t("Sonido de obturador")}
          />
        }
      />
      {/* S4 (capturas): el fondo para todas las capturas (E1). */}
      <FondoEnLasCapturas settings={settings} patch={patch} />
    </Grupo>
  );
}

/* ------------------------------------------------------------------ Grabar */

export function ApartadoGrabar({
  imprPant,
  settings,
  patch,
  shortcuts,
  camaras,
  enLinux,
  enWayland,
  teclasDelEscritorio,
  teclasAMano,
  ordenDeLaApp,
  cambiarTeclas,
  sonido,
}: DatosAjustes) {
  const t = useT();
  const { pista: pistaDeAtajo, tono: tonoDeAtajo } = usePistaDeAtajo({ teclasAMano, enWayland });
  const teclaDeGrabar = teclasDelEscritorio?.record ?? null;
  // S44 (Linux): donde no hay sonido que grabar, el interruptor sale apagado y dice por qué:
  // nunca en silencio. S45: el sonido se pide a PipeWire y, donde no lo lleva (Ubuntu 22.04,
  // Linux Mint 21), a PulseAudio; si PulseAudio está pero falta su plugin, se dice qué instalar.
  const sinSonidoAqui = sonido !== null && !sonido.altavoz && !sonido.microfono;
  const porQueSinNada = sonido?.faltaElDePulse
    ? t("para grabar el sonido falta el paquete gstreamer1.0-plugins-good")
    : t("este sistema no da el sonido por PipeWire ni por PulseAudio");
  const porQueSinAltavoz =
    sonido === null || sonido.altavoz
      ? undefined
      : sinSonidoAqui
        ? porQueSinNada
        : t("no hay ninguna salida de sonido");
  const porQueSinMicro =
    sonido === null || sonido.microfono
      ? undefined
      : sinSonidoAqui
        ? porQueSinNada
        : t("no hay ningún micrófono conectado");
  return (
    <>
      <Grupo title={t("Cómo se graba")} tour="como-se-graba">
        <Row
          id="graRegion"
          icon={<Video className="size-4" />}
          label={t("Grabar región")}
          explicacion={
            enWayland
              ? t("Recortas igual que para una captura, pero en vez de una foto sale un vídeo. En Wayland no hay barra de grabar, porque no se puede dejar fuera del vídeo: esta misma tecla es la que para.")
              : t("Recortas igual que para una captura, pero en vez de una foto sale un vídeo. El mismo atajo lo termina, así que no hace falta ir a buscar ningún botón mientras grabas.")
          }
          hint={imprPant?.screenclip ? (imprPant.porEncargo ? t("Atajo de Windows") : partesDeAtajo(GRABACION_WINDOWS).join(" + ")) : pistaDeAtajo(shortcuts.record, Boolean(teclaDeGrabar))}
          tone={tonoDeAtajo(Boolean(teclaDeGrabar))}
          control={
            imprPant?.screenclip && imprPant.porEncargo ? (
              <strong>{partesDeAtajo(GRABACION_WINDOWS).join(" + ")}</strong>
            ) : teclasAMano ? (
              <OrdenDelAtajo orden={ordenDeLaApp} accion="record" />
            ) : enWayland ? (
              <span className="flex items-center gap-1.5">
                <TeclaPuesta tecla={teclaDeGrabar} />
                <RowButton onClick={cambiarTeclas}>{t("Cambiar")}</RowButton>
              </span>
            ) : (
              <ShortcutField
                value={settings.recordShortcut}
                active={shortcuts.record}
                onChange={(v) => patch({ recordShortcut: v })}
              />
            )
          }
        />
        {/* S2 (grabación): la tecla de zoom. */}
        <FilaTeclaZoom settings={settings} patch={patch} enWayland={enWayland} />
        <Row
          id="graFps"
          icon={<Gauge className="size-4" />}
          label={t("Fotogramas por segundo")}
          explicacion={t("Cuántas veces por segundo se mira la pantalla. 30 vale para casi todo; 60 es para movimiento rápido y cuesta el doble de disco y de máquina. El editor puede bajarlo después, pero lo que no se grabó no se inventa.")}
          control={
            <Segmented
              etiqueta={t("Fotogramas por segundo")}
              value={settings.fps}
              options={FPS_OPTIONS}
              onChange={(v) => patch({ fps: v })}
            />
          }
        />
        <Row
          id="graEditor"
          icon={<SquarePen className="size-4" />}
          label={t("Abrir el editor al terminar")}
          explicacion={t("Al parar una grabación, abrirla para recortarla y exportarla. Apagado, el vídeo o el GIF se guarda solo en tu carpeta, entero y tal cual se grabó, y tú sigues a lo tuyo. Lo que rescatas de los últimos segundos abre el editor igualmente: ahí todavía no hay ningún archivo hecho.")}
          control={
            <Switch
              checked={settings.openEditorAfterRecording}
              onChange={(v) => patch({ openEditorAfterRecording: v })}
              label={t("Abrir el editor al terminar")}
            />
          }
        />
        <Row
          id="graEditorExterno"
          icon={<SquarePen className="size-4" />}
          label={t("Editor de vídeo externo")}
          hint={settings.editorExterno}
          control={
            <div className="flex items-center gap-2">
              <RowButton onClick={() => void elegirEditorExterno().then((ruta) => ruta && patch({ editorExterno: ruta }))}>
                {t("Elegir ejecutable…")}
              </RowButton>
              {settings.editorExterno && (
                <RowButton onClick={() => patch({ editorExterno: null })}>{t("Quitar editor")}</RowButton>
              )}
            </div>
          }
        />
        {settings.editorExterno && (
          <Row
            stacked
            label={t("Argumentos del editor")}
            hint={t("Un argumento por línea. Usa {file} para colocar la ruta del vídeo. Si lo omites, la ruta va al final.")}
            control={
              <textarea
                aria-label={t("Argumentos del editor")}
                className="w-full resize-y rounded-md border border-linea bg-realce px-2.5 py-1.5 font-mono text-[13px] text-titulo"
                rows={3}
                placeholder="{file}"
                defaultValue={settings.argumentosEditorExterno}
                onBlur={(evento) => {
                  const valor = evento.currentTarget.value;
                  if (valor !== settings.argumentosEditorExterno) patch({ argumentosEditorExterno: valor });
                }}
              />
            }
          />
        )}
      </Grupo>

      <Grupo title={t("Sonido y cámara")}>
        <Row
          id="graAudio"
          icon={<Volume2 className="size-4" />}
          label={t("Audio del sistema")}
          explicacion={t("Graba lo que suena por los altavoces, dentro del mismo vídeo: la voz de una llamada, el audio de un vídeo, la alerta de un programa. No graba tu micrófono, eso es la fila de abajo.")}
          hint={porQueSinAltavoz}
          tone="aviso"
          control={
            <Switch
              checked={settings.recordAudio && !porQueSinAltavoz}
              disabled={Boolean(porQueSinAltavoz)}
              onChange={(v) => patch({ recordAudio: v })}
              label={t("Audio del sistema")}
            />
          }
        />
        <Row
          id="graMicro"
          icon={<Mic className="size-4" />}
          label={t("Micrófono")}
          explicacion={t("Tu voz, en su propia pista: en el editor se sube, se baja o se quita sin tocar el sonido del sistema. Es lo que hace falta para explicar algo mientras lo enseñas, y solo se puede decidir antes de grabar.")}
          hint={porQueSinMicro}
          tone="aviso"
          control={
            <Switch
              checked={settings.recordMicrophone && !porQueSinMicro}
              disabled={Boolean(porQueSinMicro)}
              onChange={(v) => patch({ recordMicrophone: v })}
              label={t("Micrófono")}
            />
          }
        />
        {/* S10 (sonidos): la marimba al empezar a grabar. */}
        <Row
          id="graSonido"
          icon={<Bell className="size-4" />}
          label={t("Sonido al empezar")}
          explicacion={t("Una marimba corta justo antes de que empiece la grabación, para saber sin mirar que ya estás grabando. Suena y acaba antes de abrir el sonido, así que no queda dentro del vídeo.")}
          control={
            <Switch
              checked={settings.sonidoAlEmpezar ?? true}
              onChange={(v) => patch({ sonidoAlEmpezar: v })}
              label={t("Sonido al empezar")}
            />
          }
        />
        {/* S2 (grabación): la webcam. */}
        <FilasCamara settings={settings} patch={patch} camaras={camaras} enLinux={enLinux} />
      </Grupo>
    </>
  );
}

/* ------------------------------------------------------------------ Los últimos segundos */

export function ApartadoUltimos({
  settings,
  patch,
  shortcuts,
  replay,
  pantallas,
  medidaGrabada,
  enLinux,
  enWayland,
  teclasDelEscritorio,
  teclasAMano,
  ordenDeLaApp,
  cambiarTeclaDeRescate,
}: DatosAjustes) {
  const t = useT();
  const costo = LO_QUE_COSTO[`${settings.replayFps}|${settings.replayHeight}`];
  // S45 (Linux): en Wayland la tecla de rescatar la pone el escritorio, como las demás.
  const { pista: pistaDeAtajo } = usePistaDeAtajo({ teclasAMano, enWayland });
  const teclaDeRescate = teclasDelEscritorio?.replay ?? null;
  const sinTeclaDeRescate = enWayland && !teclasAMano && replay.running && !teclaDeRescate;
  // Apagado sin que nadie tocara el interruptor: la fila dice por qué, en gris.
  const porQueApagado = enLinux && !replay.running && replay.apagadoPor ? APAGADO_POR[replay.apagadoPor] : undefined;
  return (
    <>
      <Grupo>
        <Row
          id="ultSiempre"
          icon={<History className="size-4" />}
          label={t("Grabar siempre lo último")}
          explicacion={
            enWayland
              ? t("winshotx graba la pantalla todo el rato y va tirando lo viejo, así que lo que acaba de pasar sigue estando ahí aunque no le hubieras dado a grabar. Y cuesta: gasta más cuantos más fotogramas y más resolución le pidas, y cuanto más se mueva lo que hay en la pantalla. En Wayland el escritorio te pregunta qué pantalla compartes la primera vez, y mientras esté encendido enseña su aviso de que se está compartiendo la pantalla: es suyo y no se puede quitar.")
              : t("winshotx graba la pantalla todo el rato y va tirando lo viejo, así que lo que acaba de pasar sigue estando ahí aunque no le hubieras dado a grabar. Y cuesta: gasta más cuantos más fotogramas y más resolución le pidas, y cuanto más se mueva lo que hay en la pantalla.")
          }
          // Encendido, lo que de verdad cuesta tenerlo puesto: de dónde, a qué tamaño y cuánto
          // le escribe al disco cada segundo. Es un dato, en gris: no un «todo va bien» en verde.
          hint={
            replay.running
              ? t("pantalla {n} · {ancho} × {alto} · {ritmo}/s", {
                  n: replay.screen,
                  ancho: replay.width,
                  alto: replay.height,
                  ritmo: formatBytes(replay.bytesPerSecond),
                })
              : porQueApagado
                ? t(porQueApagado)
                : enWayland
                  ? t("mientras esté encendido, el escritorio avisa de que se comparte la pantalla")
                  : undefined
          }
          control={
            <Switch
              checked={settings.replayEnabled}
              onChange={(v) => patch({ replayEnabled: v })}
              label={t("Grabar siempre lo último")}
            />
          }
        />
        <OtrosGrabadores />
        {/* Solo con más de una pantalla: elegir entre una es una fila de ruido. Y en Wayland
            tampoco: ahí la pantalla se elige en la pregunta del escritorio al encenderlo. */}
        {pantallas.length > 1 && !enWayland && (
          <Row
            id="ultPantalla"
            icon={<Monitor className="size-4" />}
            label={t("Qué pantalla")}
            explicacion={t("El anillo vigila UNA pantalla y no puede cambiar a mitad, porque mudarse se llevaría por delante lo grabado. Al elegir una, su número aparece un par de segundos en esa misma pantalla para que sepas cuál es.")}
            control={
              <Segmented
                etiqueta={t("Qué pantalla")}
                value={settings.replayScreen ?? -1}
                options={[
                  { value: -1, label: t("Ratón") },
                  // La principal lleva una estrella dibujada, no la «★» de texto: un glifo
                  // cambia con la fuente y no casa con los iconos de trazo del resto.
                  ...pantallas.map((p) =>
                    p.isPrimary
                      ? {
                          value: p.id,
                          label: t("{n}, la principal", { n: p.id + 1 }),
                          icono: (
                            <span className="inline-flex items-center gap-1">
                              {p.id + 1}
                              <Star aria-hidden className="size-3" fill="currentColor" strokeWidth={0} />
                            </span>
                          ),
                        }
                      : { value: p.id, label: `${p.id + 1}` },
                  ),
                ]}
                onChange={(v) => {
                  patch({ replayScreen: v === -1 ? null : v });
                  // «La 2» no dice nada si no sabes cuál es la 2: se enseña el número EN esa
                  // pantalla, que es la única forma sin dudas.
                  if (v >= 0) void showScreenNumber(v).catch(() => {});
                }}
              />
            }
          />
        )}
        <Row
          id="ultQuedarme"
          icon={<Video className="size-4" />}
          label={t("Quedarme con lo último")}
          explicacion={
            teclasAMano
              ? t("Guarda los últimos segundos y te los abre en el editor **sin dejar de grabar**, así que puedes seguir y volver a rescatar dentro de un rato. Este escritorio no deja que las apps pidan teclas: pon esta orden en sus atajos de teclado.")
              : enWayland
                ? t("La tecla que rescata. Guarda los últimos segundos y te los abre en el editor **sin dejar de grabar**, así que puedes seguir y volver a rescatar dentro de un rato. En Wayland la tecla la elige el escritorio: te la pregunta al encender esto, y la cambias en su ventana con «Cambiar».")
                : t("La tecla que rescata. Guarda los últimos segundos y te los abre en el editor **sin dejar de grabar**, así que puedes seguir y volver a rescatar dentro de un rato.")
          }
          hint={
            !replay.running
              ? t("primero hay que encenderlo aquí arriba")
              : sinTeclaDeRescate
                ? pistaDeAtajo(false, false)
                : replay.bufferedMs < settings.replaySeconds * 1000
                  ? t("ahora mismo hay {n} s guardados", { n: Math.floor(replay.bufferedMs / 1000) })
                  : enWayland
                    ? pistaDeAtajo(true, true)
                    : shortcuts.replay
                      ? undefined
                      : t("esa combinación está ocupada")
          }
          tone={(enWayland ? sinTeclaDeRescate : replay.running && !shortcuts.replay) ? "aviso" : "dato"}
          control={
            teclasAMano ? (
              <OrdenDelAtajo orden={ordenDeLaApp} accion="replay" />
            ) : enWayland ? (
              <span className="flex items-center gap-1.5">
                {/* Con el anillo apagado la tecla no se le ha pedido al escritorio: no tenerla
                    no es un fallo, así que no va en rojo. */}
                <TeclaPuesta tecla={teclaDeRescate} sinPedir={!replay.running} />
                <RowButton onClick={cambiarTeclaDeRescate} disabled={!replay.running}>
                  {t("Cambiar")}
                </RowButton>
              </span>
            ) : (
              <ShortcutField
                value={settings.replayShortcut}
                // Con el anillo apagado la tecla NO se pide al sistema, así que pintarla en
                // rojo diría que está ocupada cuando solo está sin usar. El rojo tiene que
                // significar una sola cosa.
                active={shortcuts.replay || !replay.running}
                onChange={(v) => patch({ replayShortcut: v })}
              />
            )
          }
        />
        {/* S45 (Linux): en Wayland la tecla es del escritorio, y puede no haber ninguna (se
            cerró su pregunta, o este escritorio no deja pedirlas). Aquí se rescata igual, con
            el ratón. La fila está siempre, encendido o no, para que nada se mueva de sitio. */}
        {enWayland && (
          <Row
            icon={<Download className="size-4" />}
            label={t("Guardar ahora")}
            explicacion={t("Lo mismo que hace la tecla: guarda los últimos segundos y te los abre en el editor, sin dejar de grabar. Para cuando el escritorio no te ha dado tecla o no te acuerdas de cuál es. También lo hace la orden «winshotx --replay».")}
            control={
              <RowButton onClick={() => void replaySave().catch(() => undefined)} disabled={!replay.running}>
                {t("Guardar")}
              </RowButton>
            }
          />
        )}
      </Grupo>

      <Grupo title={t("Lo que guarda")}>
        <Row
          id="ultCuanto"
          icon={<Timer className="size-4" />}
          label={t("Cuánto se guarda")}
          explicacion={t("Cuánto tiempo hacia atrás se puede rescatar. Más segundos es más memoria y más disco dando vueltas, y hasta que no pase ese tiempo desde que lo encendiste, lo que guardes durará menos de lo que pone aquí.")}
          control={
            <Segmented
              etiqueta={t("Cuánto se guarda")}
              value={settings.replaySeconds}
              options={SEGUNDOS_ATRAS}
              onChange={(v) => patch({ replaySeconds: v })}
            />
          }
        />
        <Row
          id="ultCalidad"
          icon={<ZoomIn className="size-4" />}
          label={t("Calidad")}
          explicacion={t("A qué tamaño se guarda lo que va entrando. Bajarlo de la resolución nativa ocupa mucho menos y se nota poco en pantalla, y aquí abajo tienes lo que puede llegar a ocupar en disco con lo que hayas elegido.")}
          hint={t("{ancho} × {alto} · hasta {tamaño} en disco", {
            ancho: medidaGrabada.ancho,
            alto: medidaGrabada.alto,
            "tamaño": formatBytes(
              loQuePuedeOcupar(settings.replaySeconds, settings.replayFps, medidaGrabada.ancho, medidaGrabada.alto),
            ),
          })}
          control={
            <Segmented
              etiqueta={t("Calidad")}
              value={settings.replayHeight}
              options={ALTO_ANILLO.map((o) => ({ ...o, label: t(o.label) }))}
              onChange={(v) => patch({ replayHeight: v })}
            />
          }
        />
        <Row
          id="ultFluidez"
          icon={<Gauge className="size-4" />}
          label={t("Fluidez")}
          explicacion={t("Los fotogramas por segundo del anillo. 15 es suficiente para ver qué pasó y es el que menos molesta al ordenador; 60 se ve suave pero escribe cuatro veces más, y esto está corriendo toda la tarde.")}
          // Lo que cuesta la combinación elegida, cuando está medido. Va aquí y no en la
          // explicación del icono porque hay que verlo sin buscarlo: es lo que se paga toda
          // la tarde.
          hint={costo ? t(costo) : undefined}
          tone={settings.replayFps === 60 && settings.replayHeight === 0 ? "aviso" : "dato"}
          control={
            <Segmented
              etiqueta={t("Fluidez")}
              value={settings.replayFps}
              options={FPS_ANILLO}
              onChange={(v) => patch({ replayFps: v })}
            />
          }
        />
      </Grupo>
    </>
  );
}

/* ------------------------------------------------------------------ Teclas de Windows */

export function ApartadoTeclas({
  settings,
  shortcuts,
  imprPant,
  cambiarImprPant,
  recortes,
  quitarRecortes,
  aplicando,
  aplicarTecla,
  cambiarWinShiftS,
}: DatosAjustes) {
  const t = useT();
  // La copia de la Microsoft Store no puede con ninguna de las dos teclas: lo que escribe en
  // los ajustes de Windows se queda en su caja y Windows no lo ve (el PC de clase, 19-nov). Se
  // dice en las dos filas y lleva a la otra versión con un solo botón, en la primera.
  const tienda = imprPant?.storeBuild === true;
  if (imprPant?.screenclip) {
    return (
      <Grupo tour="las-teclas">
        <TeclasNativas estado={imprPant} legado={settings.takeWinShiftS} />
        {settings.takeWinShiftS && <Row id="tecWinS" label={t("Quedarme con Win+Mayús+S")}
          control={<Switch checked onChange={() => cambiarWinShiftS(false)} label={t("Quedarme con Win+Mayús+S")} />} />}
        {settings.printScreenCapture && <Row id="tecImpr" label={t("Usar también Impr Pant")}
          control={<Switch checked onChange={() => cambiarImprPant(false)} label={t("Usar también Impr Pant")} />} />}
        <Row id="tecRecortesWindows" label={t("Windows abre winshotx al pulsar los atajos de recorte.")}
          control={<RowButton onClick={() => void configurarRecortesDeWindows().catch(console.error)}>{t("Aplicaciones predeterminadas…")}</RowButton>} />
      </Grupo>
    );
  }
  return (
    <Grupo tour="las-teclas">
      {!tienda && (
        <Row
          id="tecRecortesWindows"
          icon={<AppWindow className="size-4" />}
          label={t("Win+Mayús+S y Win+Mayús+R")}
          hint={
            settings.takeWinShiftS
              ? t("Desactiva la opción antigua de Win+Mayús+S y vuelve a iniciar sesión.")
              : t("Elige winshotx para MS-SCREENCLIP. Windows abrirá la captura o la grabación bajo demanda.")
          }
          control={
            <RowButton onClick={() => void configurarRecortesDeWindows().catch(console.error)}>
              {t("Aplicaciones predeterminadas…")}
            </RowButton>
          }
        />
      )}
      <Row
        id="tecImpr"
        icon={<Keyboard className="size-4" />}
        label={t("Impr Pant")}
        explicacion={
          tienda
            ? t("Windows guarda aparte lo que la versión de la Microsoft Store cambia en sus ajustes, y su tecla no se entera. La versión de winshotx.com, o la de winget, sí puede quedársela.")
            : t("Windows le tiene asignada la Herramienta de Recortes, y esto se la quita para dársela a winshotx. Si la tecla no responde, hay que cerrar sesión una vez para que Windows la suelte del todo.")
        }
        // Solo lo que pide algo: que Windows todavía no la ha soltado. «Ya es de winshotx» es
        // un «todo va bien» que no pide nada, y lo que hace la tecla ya lo dice el dibujo. Que
        // Windows haya dejado registrarla no basta: con Impr Pant dice que sí aunque la siga
        // teniendo la Herramienta de Recortes, así que manda también lo que hay en el registro.
        hint={
          tienda
            ? t("la versión de la Microsoft Store no puede quitársela a Windows")
            : imprPant?.enabled && (!imprPant.active || imprPant.takenByWindows)
              ? t("cierra sesión para que Windows la suelte")
              : undefined
        }
        tone="aviso"
        control={
          <span className="flex items-center gap-2">
            {tienda && <RowButton onClick={() => void openUrl(WEB)}>{t("Bajar de winshotx.com")}</RowButton>}
            <Switch
              checked={!tienda && (imprPant?.enabled ?? false)}
              disabled={tienda}
              onChange={(v) => void cambiarImprPant(v)}
              label={t("Usar también Impr Pant")}
            />
          </span>
        }
      />
      <Row
        id="tecWinS"
        icon={<AppWindow className="size-4" />}
        label={t("Win + Mayús + S")}
        explicacion={t("La otra tecla de captura de Windows. Cogerla cuesta Win+S, la búsqueda, porque las dos se registran juntas; y si el Explorador sigue abriendo lo suyo, con Aplicar se reinicia y la suelta.")}
        // Lo que cuesta va antes de pulsar, no después: es lo único que hay que saber para
        // decidir.
        hint={
          tienda
            ? t("tampoco desde la Store: hace falta la de winshotx.com")
            : !settings.takeWinShiftS
              ? t("cuesta Win+S, la búsqueda")
              : aplicando
                ? t("reiniciando el Explorador…")
                : shortcuts.winShiftS
                  ? t("de winshotx, o pulsa Aplicar si Windows la sigue abriendo")
                  : t("el escritorio todavía la tiene")
        }
        tone={tienda || (settings.takeWinShiftS && !aplicando && !shortcuts.winShiftS) ? "aviso" : "dato"}
        control={
          <span className="flex items-center gap-2">
            {/*
              Cuando el atajo de captura del usuario ya es Win+Mayus+S, el backend lo marca
              como conseguido en cuanto RegisterHotKey tiene exito, sin poder saber si el
              Explorador (que intercepta esta combinacion antes que cualquier programa) ya
              releyo la lista de teclas apagadas. El boton se deja siempre visible: reiniciar
              el Explorador no hace dano de mas, y es la unica forma real de confirmarlo.
              Desde la Store no sale: el Explorador nuevo leeria el registro sin la S.
            */}
            {settings.takeWinShiftS && !tienda && (
              <RowButton
                disabled={aplicando}
                onClick={() => void aplicarTecla()}
                title={t("Reinicia el Explorador para que Windows suelte la tecla. La barra de tareas parpadea un segundo y no se cierra nada más.")}
              >
                {aplicando ? t("Un momento…") : t("Aplicar")}
              </RowButton>
            )}
            <Switch
              checked={!tienda && settings.takeWinShiftS}
              disabled={tienda}
              onChange={(v) => void cambiarWinShiftS(v)}
              label={t("Quedarme con Win+Mayús+S")}
            />
          </span>
        }
      />
      <Row
        id="tecRecortes"
        icon={<Scissors className="size-4" />}
        label={t("Herramienta de Recortes")}
        explicacion={t("Desactivar sus teclas no la calla del todo: sigue saliendo desde el menú de inicio y desde otras teclas. Quitarla es lo único definitivo, y se puede volver a instalar desde la Microsoft Store cuando quieras.")}
        hint={
          recortes === "confirmar"
            ? t("vuelve desde la Microsoft Store")
            : recortes === "quitando"
              ? t("quitándola…")
              : recortes === "quitada"
                ? t("quitada, ya no abre nada")
                : recortes === "ya no estaba"
                  ? t("no estaba instalada")
                  : undefined
        }
        tone={recortes === "confirmar" ? "aviso" : "dato"}
        control={
          <span className="flex gap-2">
            {!recortes && <RowButton onClick={() => void openWindowsApps()}>{t("Ver cómo")}</RowButton>}
            <RowButton
              danger={recortes === "confirmar"}
              disabled={recortes === "quitando" || recortes === "quitada" || recortes === "ya no estaba"}
              onClick={() => void quitarRecortes()}
            >
              {recortes === "confirmar"
                ? t("Sí, quitarla")
                : recortes === "quitada" || recortes === "ya no estaba"
                  ? t("Hecho")
                  : t("Quitar")}
            </RowButton>
          </span>
        }
      />
    </Grupo>
  );
}

/* ------------------------------------------------------------------ La app */

export function ApartadoApp({ settings, patch, cache, vaciarCache, enLinux, imprPant }: DatosAjustes) {
  const t = useT();
  const temas = TEMAS.map((o) => ({ ...o, label: t(o.label) }));
  const idiomas = IDIOMAS.map((o) => ({ ...o, label: t(o.label) }));
  const arrancar = enLinux ? t("Arrancar al iniciar sesión") : t("Arrancar con Windows");
  return (
    <Grupo tour="archivos">
      <Row
        id="appCarpeta"
        icon={<FolderOpen className="size-4" />}
        label={t("Carpeta de destino")}
        explicacion={t("Donde caen las capturas y los vídeos que guardas. El nombre lo pone winshotx con la fecha y la hora, y si ya existiera uno igual no lo pisa nunca.")}
        hint={settings.saveDirectory}
        control={
          <span className="flex gap-2">
            <RowButton onClick={() => void openFolder(settings.saveDirectory)}>{t("Abrir")}</RowButton>
            <RowButton
              onClick={() => void pickDirectory().then((dir) => dir && patch({ saveDirectory: dir }))}
            >
              {t("Cambiar")}
            </RowButton>
          </span>
        }
      />
      <Row
        id="appCache"
        icon={<HardDrive className="size-4" />}
        label={t("Caché de grabaciones")}
        explicacion={t("Los fotogramas en crudo de lo que has grabado, que es lo que permite volver a exportar sin perder calidad. Vaciarlo no toca ni una captura guardada: solo tira lo que quedó a medias en el editor.")}
        hint={
          cache.sessions === 0
            ? t("vacía")
            : t("{tam} en {n} {palabra}", {
                tam: formatBytes(cache.bytes),
                n: cache.sessions,
                palabra: cache.sessions === 1 ? t("sesión") : t("sesiones"),
              })
        }
        control={
          <RowButton disabled={cache.sessions === 0} onClick={vaciarCache}>
            {t("Vaciar")}
          </RowButton>
        }
      />
      <Row
        id="appArrancar"
        icon={<Power className="size-4" />}
        label={arrancar}
        explicacion={
          imprPant?.screenclip ? t("Windows abre winshotx al pulsar los atajos de recorte.") : enLinux
            ? t("winshotx se abre solo al entrar en tu sesión y se queda esperando, sin ventana. Sin esto hay que abrirlo a mano cada vez, y las teclas de captura no funcionan mientras no esté abierto.")
            : t("winshotx se abre solo al encender el ordenador y se queda en la bandeja, sin ventana. Sin esto hay que abrirlo a mano cada vez, y las teclas de captura no funcionan mientras no esté abierto.")
        }
        control={
          <Switch
            checked={settings.startWithWindows}
            onChange={(v) => patch({ startWithWindows: v })}
            label={arrancar}
          />
        }
      />
      <Row
        id="appTema"
        icon={<Palette className="size-4" />}
        label={t("Tema")}
        explicacion={
          enLinux
            ? t("Claro, oscuro, o lo que diga el sistema y cambiar con él. Es solo el color de estas ventanas; lo que capturas no cambia.")
            : t("Claro, oscuro, o lo que diga Windows y cambiar con él. Es solo el color de estas ventanas; lo que capturas no cambia.")
        }
        control={
          <Segmented
            etiqueta={t("Tema")}
            value={settings.theme}
            options={temas}
            onChange={(v) => {
              // Se pinta en el acto y se guarda despues: esperar a que Rust conteste para
              // cambiar de color hace que el boton parezca roto.
              aplicarTema(v);
              patch({ theme: v });
            }}
          />
        }
      />
      <Row
        id="appIdioma"
        stacked
        icon={<Languages className="size-4" />}
        label={t("Idioma")}
        explicacion={
          enLinux
            ? t("Español, inglés, o el del sistema si winshotx lo habla. Cambia al momento y no hace falta reiniciar nada.")
            : t("Español, inglés, o el de Windows si winshotx lo habla. Cambia al momento y no hace falta reiniciar nada.")
        }
        control={
          <Segmented
            etiqueta={t("Idioma")}
            value={settings.language}
            options={idiomas}
            onChange={(v) => {
              // Igual que el tema: se cambia el idioma en el acto y se guarda despues. Los
              // textos se repintan solos, sin recargar la ventana ni perder por donde iba.
              aplicarIdioma(v);
              patch({ language: v });
            }}
          />
        }
      />
    </Grupo>
  );
}

/* ------------------------------------------------------------------ Ayuda y winshotx */

export function ApartadoAyuda({ onVerBienvenida, onTour, version, actualizacion, enLinux }: DatosAjustes) {
  const t = useT();
  const { fase, texto } = actualizacion;
  // Si lo ha pedido él con el botón. «Estás en la última versión» es la respuesta a esa
  // pregunta; sin preguntar (la ventana mira sola al abrirse) sería un «todo va bien» que no
  // pide nada.
  const [pedido, setPedido] = useState(false);
  // La línea de la versión: lo de actualizar cuando hay algo que contar, y si no, a dónde
  // lleva «La web».
  const conNoticia = fase.tipo === "aldia" ? pedido : fase.tipo !== "quieto";
  return (
    <>
      <Grupo>
        <Row
          id="appBienvenida"
          icon={<BookOpen className="size-4" />}
          label={t("Bienvenida")}
          explicacion={
            enLinux
              ? t("Las tres pantallas del primer día, otra vez: lo que hace cada tecla y qué elegir. Dura menos de un minuto y no cambia ningún ajuste.")
              : t("Las cuatro pantallas del primer día, otra vez: lo que hace cada tecla y qué elegir. Dura menos de un minuto y no cambia ningún ajuste.")
          }
          control={<RowButton onClick={onVerBienvenida}>{t("Ver otra vez")}</RowButton>}
        />
        <Row
          id="appTour"
          icon={<Compass className="size-4" />}
          label={t("Tour de los ajustes")}
          explicacion={t("Recorre esta pantalla parándose en cada bloque y contando para qué sirve. Es la vía rápida para ver lo que hay aquí sin ir abriendo cosas a ver qué pasa.")}
          control={<RowButton onClick={onTour}>{t("Empezar")}</RowButton>}
        />
      </Grupo>
      {/* Lo unico que winshotx pide, y va al final del todo a proposito: primero la
          herramienta funcionando y solo despues quien la hace. Debajo de cada boton que se
          lleva fuera va la direccion escrita, porque un boton que abre el navegador tiene
          que decir a donde antes de que lo pulsen. */}
      <Grupo title={t("Acerca de")} tour="acerca">
        <Row
          id="appVersion"
          icon={<Info className="size-4" />}
          label={t("winshotx {version}", { version })}
          explicacion={t("Gratis, sin cuentas y sin anuncios, con licencia MIT. Lo hago yo solo, y ni las capturas ni los vídeos salen nunca de tu ordenador: aquí no hay servidor al que mandarlos.")}
          hint={conNoticia && texto ? texto : comoSeLee(WEB)}
          tone={fase.tipo === "error" ? "error" : "dato"}
          control={
            <span className="flex items-center gap-2">
              <BotonActualizar
                fase={fase}
                instalar={actualizacion.instalar}
                mirar={() => {
                  setPedido(true);
                  actualizacion.mirar();
                }}
              />
              <RowButton onClick={() => void openUrl(WEB)}>{t("La web")}</RowButton>
            </span>
          }
        />
        <Row
          id="appCafe"
          icon={<Coffee className="size-4" />}
          label={t("Invítame a un café")}
          explicacion={t("Es la única forma de apoyar winshotx: no hay versión de pago, ni anuncios, ni datos que vender. Se paga una vez, la cantidad la pones tú, y no hace falta cuenta.")}
          hint={comoSeLee(CAFE)}
          control={<RowButton onClick={() => void openUrl(CAFE)}>{t("Invitar")}</RowButton>}
        />
        <Row
          id="appCodigo"
          icon={<Github className="size-4" />}
          label={t("El código y los fallos")}
          explicacion={t("Ayuda igual que un café y es gratis: una estrella hace que lo encuentre más gente, y un fallo contado con lo que estabas haciendo es lo que hace que se arregle.")}
          hint={t("abierto, con licencia MIT")}
          control={
            <span className="flex gap-2">
              <RowButton onClick={() => void openUrl(REPO)}>{t("El código")}</RowButton>
              <RowButton onClick={() => void openUrl(FALLOS)}>{t("Contar un fallo")}</RowButton>
            </span>
          }
        />
      </Grupo>
    </>
  );
}
