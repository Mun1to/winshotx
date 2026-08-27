import { useCallback, useEffect, useState } from "react";
import type { ReactNode } from "react";
import { AnimatePresence, motion } from "framer-motion";
import {
  Camera,
  Check,
  Clipboard,
  Keyboard,
  MousePointerClick,
  MousePointer2,
  Power,
  Scissors,
  Video,
} from "lucide-react";
import { getSettings, setSettings, shortcutStatus, usePrintScreen } from "../../lib/ipc";
import type {
  CaptureFlow,
  PrintScreenState,
  Settings,
  ShortcutStatus,
} from "../../lib/types";
import { partesDeAtajo } from "../../lib/teclas";
import { ShortcutField } from "../settings/ShortcutField";
import { Marca } from "../ui/Marca";
import { Switch } from "../ui/Switch";
import { useT } from "../../lib/i18n";

/**
 * La bienvenida, una sola vez. Se abre sola tras instalar porque winshotx vive en la
 * bandeja: sin esto, la primera impresion de la app es que no ha pasado nada.
 *
 * Los cuatro pasos son las decisiones que no se pueden adivinar por el usuario: como
 * quiere capturar, si le cede la tecla Impr Pant a winshotx y si quiere que arranque con
 * Windows. Todo lo demas trae un valor por defecto que ya funciona.
 */

const PASOS = ["Hola", "Estilo", "Impr Pant", "Listo"];

/**
 * Combinaciones recomendadas. Elegidas por no chocar con lo que ya usa Windows ni con lo
 * que usan a diario el navegador y el editor: `Ctrl+Shift+2` y `Ctrl+Shift+5` son las de
 * fabrica, `Ctrl+Alt+letra` casi nunca esta cogida, y `Alt+letra` es la mas corta de teclear.
 */
const SUGERIDOS_CAPTURA = ["CmdOrCtrl+Shift+Digit2", "CmdOrCtrl+Alt+KeyA", "Alt+KeyX"];
const SUGERIDOS_GRABACION = ["CmdOrCtrl+Shift+Digit5", "CmdOrCtrl+Alt+KeyR", "Alt+KeyV"];

export function WelcomeApp({ onDone }: { onDone: () => void }) {
  const t = useT();
  const [paso, setPaso] = useState(0);
  const [ajustes, setAjustes] = useState<Settings | null>(null);
  const [imprPant, setImprPant] = useState<PrintScreenState | null>(null);
  const [atajos, setAtajos] = useState<ShortcutStatus>({
    capture: true,
    record: true,
    printScreen: false,
    winShiftS: false,
  });
  const [ocupado, setOcupado] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    void getSettings().then(setAjustes);
    void shortcutStatus().then(setAtajos);
  }, []);

  /** Se guarda en cuanto se toca algo: si la ventana se cierra a medias, no se pierde. */
  const guardar = useCallback((parcial: Partial<Settings>) => {
    setAjustes((previo) => {
      if (!previo) return previo;
      const siguiente = { ...previo, ...parcial };
      void setSettings(siguiente)
        .then(() => shortcutStatus().then(setAtajos))
        .catch((e) => setError(String(e)));
      return siguiente;
    });
  }, []);

  const pedirTecla = useCallback(async (quiere: boolean) => {
    setOcupado(true);
    setError(null);
    try {
      setImprPant(await usePrintScreen(quiere));
      // El comando guarda los ajustes en Rust por su cuenta: hay que releerlos o el
      // siguiente cambio de este mismo asistente reenviaria el valor viejo.
      setAjustes(await getSettings());
    } catch (e) {
      setError(String(e));
    } finally {
      setOcupado(false);
    }
  }, []);

  const terminar = useCallback(() => {
    setAjustes((previo) => {
      if (previo) void setSettings({ ...previo, onboarded: true }).catch(() => undefined);
      return previo;
    });
    onDone();
  }, [onDone]);

  if (!ajustes) {
    return <div className="h-full bg-lienzo" />;
  }

  return (
    <div className="flex h-full flex-col overflow-hidden bg-lienzo text-texto">
      <div className="flex-1 px-9 pt-9">
        <AnimatePresence mode="wait" initial={false}>
          <motion.div
            key={paso}
            initial={{ opacity: 0, x: 14 }}
            animate={{ opacity: 1, x: 0 }}
            exit={{ opacity: 0, x: -14 }}
            transition={{ duration: 0.16, ease: "easeOut" }}
            className="h-full"
          >
            {paso === 0 && (
              <Hola
                ajustes={ajustes}
                atajos={atajos}
                onCaptura={(v) => guardar({ captureShortcut: v })}
                onGrabacion={(v) => guardar({ recordShortcut: v })}
              />
            )}
            {paso === 1 && (
              <Estilo valor={ajustes.captureFlow} onChange={(v) => guardar({ captureFlow: v })} />
            )}
            {paso === 2 && (
              <TeclaImprPant
                estado={imprPant}
                ocupado={ocupado}
                onElegir={(quiere) => void pedirTecla(quiere)}
              />
            )}
            {paso === 3 && (
              <Final
                ajustes={ajustes}
                imprPant={imprPant}
                onArranque={(v) => guardar({ startWithWindows: v })}
              />
            )}
          </motion.div>
        </AnimatePresence>
      </div>

      {error && (
        <p className="mx-9 mb-2 rounded-lg bg-red-500/10 px-3 py-2 text-[11px] text-red-300">
          {error}
        </p>
      )}

      <footer className="flex shrink-0 items-center justify-between border-t border-linea px-6 py-3">
        <span className="flex items-center gap-1.5">
          {PASOS.map((nombre, i) => (
            <span
              key={nombre}
              aria-hidden="true"
              className={`h-1.5 rounded-full transition-all duration-200 ${
                i === paso
                  ? "w-5 bg-blue-500"
                  : i < paso
                    ? "w-1.5 bg-blue-500/50"
                    : "w-1.5 bg-activo"
              }`}
            />
          ))}
          <span className="ml-2 text-[11px] text-tenue">
            {t("Paso {n} de {total}", { n: paso + 1, total: PASOS.length })}
          </span>
        </span>

        <span className="flex items-center gap-2">
          {paso > 0 && (
            <button
              type="button"
              onClick={() => setPaso((p) => p - 1)}
              className="rounded-lg px-3 py-1.5 text-xs text-apagado transition-colors hover:bg-realce hover:text-titulo"
            >
              {t("Atrás")}
            </button>
          )}
          {paso < PASOS.length - 1 ? (
            <button
              type="button"
              onClick={() => setPaso((p) => p + 1)}
              className="rounded-lg bg-blue-600 px-4 py-1.5 text-xs font-medium text-white transition-colors hover:bg-blue-500"
            >
              {paso === 0 ? t("Empezar") : t("Siguiente")}
            </button>
          ) : (
            <button
              type="button"
              onClick={terminar}
              className="rounded-lg bg-blue-600 px-4 py-1.5 text-xs font-medium text-white transition-colors hover:bg-blue-500"
            >
              {t("Todo listo")}
            </button>
          )}
        </span>
      </footer>
    </div>
  );
}

function Titulo({ texto, sub }: { texto: string; sub: string }) {
  return (
    <>
      <h1 className="text-[26px] leading-tight font-semibold tracking-tight text-titulo">{texto}</h1>
      <p className="mt-1.5 max-w-xl text-[13px] leading-relaxed text-apagado">{sub}</p>
    </>
  );
}

function Tecla({ children }: { children: string }) {
  return (
    <kbd className="rounded-md border border-linea-fuerte bg-realce px-1.5 py-0.5 font-mono text-[11px] text-texto">
      {children}
    </kbd>
  );
}

function Atajo({ valor }: { valor: string }) {
  return (
    <span className="flex items-center gap-1">
      {partesDeAtajo(valor).map((parte, i) => (
        <span key={parte + String(i)} className="flex items-center gap-1">
          {i > 0 && <span className="text-[10px] text-fantasma">+</span>}
          <Tecla>{parte}</Tecla>
        </span>
      ))}
    </span>
  );
}

/** Una combinacion recomendada, para no obligar a nadie a inventarse una. */
function Sugerencias({
  opciones,
  valor,
  onElegir,
}: {
  opciones: string[];
  valor: string;
  onElegir: (valor: string) => void;
}) {
  const t = useT();
  return (
    <span className="mt-2.5 flex flex-wrap items-center gap-1.5">
      <span className="text-[11px] text-fantasma">{t("o prueba")}</span>
      {opciones.map((opcion) => {
        const puesta = opcion === valor;
        return (
          <button
            key={opcion}
            type="button"
            onClick={() => onElegir(opcion)}
            aria-pressed={puesta}
            className={`rounded-md border px-1.5 py-0.5 font-mono text-[11px] transition-colors ${
              puesta
                ? "border-blue-500/70 bg-blue-500/15 text-blue-200"
                : "border-linea-fuerte text-apagado hover:border-linea-viva hover:text-titulo"
            }`}
          >
            {partesDeAtajo(opcion).join(" ")}
          </button>
        );
      })}
    </span>
  );
}

function Ocupado({ visible }: { visible: boolean }) {
  const t = useT();
  return (
    <span aria-live="polite" className="mt-2 block min-h-[15px] text-[11px] text-amber-300">
      {visible ? t("Esta la tiene otra aplicación. Prueba con otra.") : ""}
    </span>
  );
}

function Hola({
  ajustes,
  atajos,
  onCaptura,
  onGrabacion,
}: {
  ajustes: Settings;
  atajos: ShortcutStatus;
  onCaptura: (valor: string) => void;
  onGrabacion: (valor: string) => void;
}) {
  const t = useT();
  return (
    <div>
      <Marca className="mb-4 size-12" />
      <Titulo
        texto={t("winshotx ya está en marcha")}
        sub={t("Vive en la bandeja del sistema, junto al reloj. No hay ventana que dejar abierta: se llama con una tecla, hace lo suyo y desaparece.")}
      />

      <p className="mt-4 text-[12px] text-apagado">
        {t("Estas son las dos teclas con las que se llama. Pulsa el campo y teclea la combinación que quieras si prefieres otras.")}
      </p>

      <div className="mt-3 grid grid-cols-2 gap-3">
        <div className="rounded-xl border border-linea bg-tarjeta p-4">
          <span className="flex items-center gap-2 text-[13px] font-medium text-texto">
            <Camera className="size-4 text-tenue" />
            {t("Capturar una región")}
          </span>
          <span className="mt-2.5 flex">
            <ShortcutField
              value={ajustes.captureShortcut}
              active={atajos.capture}
              onChange={onCaptura}
            />
          </span>
          <Sugerencias
            opciones={SUGERIDOS_CAPTURA}
            valor={ajustes.captureShortcut}
            onElegir={onCaptura}
          />
          <Ocupado visible={!atajos.capture} />
        </div>
        <div className="rounded-xl border border-linea bg-tarjeta p-4">
          <span className="flex items-center gap-2 text-[13px] font-medium text-texto">
            <Video className="size-4 text-tenue" />
            {t("Grabar en GIF o vídeo")}
          </span>
          <span className="mt-2.5 flex">
            <ShortcutField
              value={ajustes.recordShortcut}
              active={atajos.record}
              onChange={onGrabacion}
            />
          </span>
          <Sugerencias
            opciones={SUGERIDOS_GRABACION}
            valor={ajustes.recordShortcut}
            onElegir={onGrabacion}
          />
          <Ocupado visible={!atajos.record} />
        </div>
      </div>

      <p className="mt-3 text-[12px] text-tenue">
        {t("Todo se queda en tu ordenador: sin cuenta, sin nube y sin nada que subir.")}
      </p>
    </div>
  );
}

interface TarjetaProps {
  elegido: boolean;
  onClick: () => void;
  titulo: string;
  resumen: string;
  pasos: { icono: ReactNode; texto: string }[];
  nota: string;
}

function TarjetaEstilo({ elegido, onClick, titulo, resumen, pasos, nota }: TarjetaProps) {
  return (
    <button
      type="button"
      onClick={onClick}
      aria-pressed={elegido}
      className={`relative flex flex-col rounded-2xl border p-4 text-left transition-colors ${
        elegido
          ? "border-blue-500/70 bg-blue-500/10"
          : "border-linea bg-tarjeta hover:border-linea-viva"
      }`}
    >
      <span
        aria-hidden="true"
        className={`absolute top-3.5 right-3.5 flex size-5 items-center justify-center rounded-full border transition-colors ${
          elegido ? "border-blue-500 bg-blue-500 text-white" : "border-linea-viva"
        }`}
      >
        {elegido && <Check className="size-3" />}
      </span>

      <span className="block text-[15px] font-semibold text-titulo">{titulo}</span>
      <span className="mt-0.5 block text-[12px] text-apagado">{resumen}</span>

      <span className="mt-3.5 mb-3.5 block space-y-2">
        {pasos.map((paso, i) => (
          <span key={paso.texto} className="flex items-center gap-2.5">
            <span className="flex size-6 shrink-0 items-center justify-center rounded-lg bg-realce text-suave">
              {paso.icono}
            </span>
            <span className="text-[12px] text-suave">
              <span className="mr-1 text-fantasma">{i + 1}.</span>
              {paso.texto}
            </span>
          </span>
        ))}
      </span>

      <span className="mt-auto block border-t border-linea pt-2.5 text-[11px] text-tenue">
        {nota}
      </span>
    </button>
  );
}

function Estilo({ valor, onChange }: { valor: CaptureFlow; onChange: (v: CaptureFlow) => void }) {
  const t = useT();
  return (
    <div>
      <Titulo
        texto={t("¿Cómo prefieres capturar?")}
        sub={t("Las dos formas usan el mismo atajo y la misma selección. Lo que cambia es lo que pasa al soltar el ratón, y se puede cambiar cuando quieras desde los ajustes.")}
      />

      <div className="mt-5 grid grid-cols-2 gap-3">
        <TarjetaEstilo
          elegido={valor === "toolbar"}
          onClick={() => onChange("toolbar")}
          titulo={t("Con barra")}
          resumen={t("Seleccionas y eliges qué hacer.")}
          pasos={[
            { icono: <Keyboard className="size-3.5" />, texto: t("Pulsas el atajo") },
            { icono: <MousePointer2 className="size-3.5" />, texto: t("Arrastras la región") },
            {
              icono: <MousePointerClick className="size-3.5" />,
              texto: t("Copiar, guardar, editar o grabar"),
            },
          ]}
          nota={t("La opción completa: de esa barra salen el editor, el GIF y el vídeo.")}
        />
        <TarjetaEstilo
          elegido={valor === "instant"}
          onClick={() => onChange("instant")}
          titulo={t("Al vuelo")}
          resumen={t("Seleccionas y se copia sola.")}
          pasos={[
            { icono: <Keyboard className="size-3.5" />, texto: t("Pulsas el atajo") },
            { icono: <MousePointer2 className="size-3.5" />, texto: t("Arrastras la región") },
            { icono: <Clipboard className="size-3.5" />, texto: t("Ya está en el portapapeles") },
          ]}
          nota={t("Para pegar en un chat sin pensar. El atajo de grabar sigue sacando la barra.")}
        />
      </div>
    </div>
  );
}

function TeclaImprPant({
  estado,
  ocupado,
  onElegir,
}: {
  estado: PrintScreenState | null;
  ocupado: boolean;
  onElegir: (quiere: boolean) => void;
}) {
  const t = useT();
  return (
    <div>
      <Titulo
        texto={t("¿Le quitamos la tecla a la Herramienta de Recortes?")}
        sub={t("En Windows, la tecla Impr Pant abre la Herramienta de Recortes. Si quieres, winshotx se queda con ella y responde a ese mismo dedo, sin aprender ningún atajo nuevo.")}
      />

      <div className="mt-5 grid grid-cols-2 gap-3">
        <button
          type="button"
          disabled={ocupado}
          onClick={() => onElegir(true)}
          aria-pressed={estado?.enabled === true}
          className={`rounded-2xl border p-4 text-left transition-colors disabled:opacity-50 ${
            estado?.enabled
              ? "border-blue-500/70 bg-blue-500/10"
              : "border-linea bg-tarjeta hover:border-linea-viva"
          }`}
        >
          <span className="flex items-center gap-2">
            <Tecla>{t("Impr Pant")}</Tecla>
            <span aria-hidden="true" className="text-fantasma">
              →
            </span>
            <span className="text-[13px] font-semibold text-titulo">winshotx</span>
          </span>
          <span className="mt-2 block text-[12px] text-apagado">
            {t("Apaga el ajuste de Windows que le da esa tecla a la Herramienta de Recortes y se la pasa a winshotx. No te quita nada más.")}
          </span>
        </button>

        <button
          type="button"
          disabled={ocupado}
          onClick={() => onElegir(false)}
          aria-pressed={estado !== null && !estado.enabled}
          className={`rounded-2xl border p-4 text-left transition-colors disabled:opacity-50 ${
            estado !== null && !estado.enabled
              ? "border-blue-500/70 bg-blue-500/10"
              : "border-linea bg-tarjeta hover:border-linea-viva"
          }`}
        >
          <span className="flex items-center gap-2">
            <Scissors className="size-4 text-tenue" />
            <span className="text-[13px] font-semibold text-titulo">{t("Dejarla como está")}</span>
          </span>
          <span className="mt-2 block text-[12px] text-apagado">
            {t("La Herramienta de Recortes se queda con Impr Pant y winshotx se llama con su atajo.")}
          </span>
        </button>
      </div>

      <div aria-live="polite" className="mt-4 min-h-[52px] text-[12px]">
        {estado?.enabled && (
          <ul className="space-y-1">
            <li className={estado.active ? "text-emerald-400" : "text-amber-300"}>
              {estado.active
                ? t("Impr Pant abre winshotx.")
                : t("Impr Pant no ha caído: hay otro programa que la tiene cogida.")}
            </li>
            <li className="text-tenue">
              {t("Si Windows sigue abriendo la Herramienta de Recortes con Impr Pant, cierra sesión y vuelve a entrar.")}
            </li>
          </ul>
        )}
        {estado !== null && !estado.enabled && (
          <p className="text-tenue">
            {t("Sin cambios. Puedes activarlo más adelante en Ajustes, en “Atajos globales”.")}
          </p>
        )}
      </div>

      <p className="mt-1 text-[11px] leading-relaxed text-tenue">
        {/* Las negritas de dentro de la frase se han quitado a proposito: dejarlas obliga
            a partir el parrafo en cinco cachos, y una frase cosida a trozos solo se puede
            traducir bien por casualidad, porque el orden de las palabras cambia. */}
        <b className="font-medium text-apagado">{t("Win + Mayús + S es otra historia.")}</b>{" "}
        {t(
          "Esa la atiende Windows antes que cualquier programa y solo se le quita apagando la S de sus atajos, lo que apaga también Win + S, la búsqueda. Por eso va aparte, en Ajustes → Atajos globales, y no entra aquí de propina.",
        )}
      </p>
    </div>
  );
}

function Final({
  ajustes,
  imprPant,
  onArranque,
}: {
  ajustes: Settings;
  imprPant: PrintScreenState | null;
  onArranque: (valor: boolean) => void;
}) {
  const t = useT();
  return (
    <div>
      <span className="mb-4 flex size-11 items-center justify-center rounded-2xl bg-emerald-500/15 text-emerald-400">
        <Check className="size-5" />
      </span>
      <Titulo
        texto={t("Listo, ya puedes capturar")}
        sub={t("Esto es lo que queda configurado. Todo se cambia después desde el icono de la bandeja.")}
      />

      <div className="mt-5 divide-y divide-linea-tenue overflow-hidden rounded-xl border border-linea bg-tarjeta">
        <div className="flex items-center justify-between px-4 py-2.5">
          <span className="text-[13px] text-suave">{t("Capturar una región")}</span>
          <span className="flex items-center gap-2">
            <Atajo valor={ajustes.captureShortcut} />
            {imprPant?.enabled && imprPant.active && (
              <>
                <span className="text-[10px] text-fantasma">o</span>
                <Tecla>{t("Impr Pant")}</Tecla>
              </>
            )}
          </span>
        </div>
        <div className="flex items-center justify-between px-4 py-2.5">
          <span className="text-[13px] text-suave">{t("Al soltar el ratón")}</span>
          <span className="text-[12px] text-apagado">
            {ajustes.captureFlow === "instant"
              ? t("se copia al portapapeles")
              : t("sale la barra para elegir")}
          </span>
        </div>
        <div className="flex items-center justify-between px-4 py-2.5">
          <span className="flex items-center gap-2.5">
            <Power className="size-4 text-tenue" />
            <span>
              <span className="block text-[13px] text-suave">{t("Arrancar con Windows")}</span>
              <span className="block text-[11px] text-tenue">
                {t("se abre en la bandeja, sin ventana")}
              </span>
            </span>
          </span>
          <Switch
            checked={ajustes.startWithWindows}
            onChange={onArranque}
            label={t("Arrancar con Windows")}
          />
        </div>
      </div>

      <p className="mt-4 text-[12px] text-tenue">
        {t("Pulsa el atajo cuando quieras. Con el botón derecho en el icono de la bandeja se abren los ajustes.")}
      </p>
    </div>
  );
}
