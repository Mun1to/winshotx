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
  Sparkles,
  Video,
  Zap,
} from "lucide-react";
import { getSettings, setSettings, usePrintScreen } from "../../lib/ipc";
import type { CaptureFlow, PrintScreenState, Settings } from "../../lib/types";
import { Switch } from "../ui/Switch";

/**
 * La bienvenida, una sola vez. Se abre sola tras instalar porque winshotx vive en la
 * bandeja: sin esto, la primera impresion de la app es que no ha pasado nada.
 *
 * Los cuatro pasos son las decisiones que no se pueden adivinar por el usuario: como
 * quiere capturar, si le cede la tecla Impr Pant a winshotx y si quiere que arranque con
 * Windows. Todo lo demas trae un valor por defecto que ya funciona.
 */

const PASOS = ["Hola", "Estilo", "Impr Pant", "Listo"];

export function WelcomeApp({ onDone }: { onDone: () => void }) {
  const [paso, setPaso] = useState(0);
  const [ajustes, setAjustes] = useState<Settings | null>(null);
  const [imprPant, setImprPant] = useState<PrintScreenState | null>(null);
  const [ocupado, setOcupado] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    void getSettings().then(setAjustes);
  }, []);

  /** Se guarda en cuanto se toca algo: si la ventana se cierra a medias, no se pierde. */
  const guardar = useCallback((parcial: Partial<Settings>) => {
    setAjustes((previo) => {
      if (!previo) return previo;
      const siguiente = { ...previo, ...parcial };
      void setSettings(siguiente).catch((e) => setError(String(e)));
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
    return <div className="h-full bg-[#161618]" />;
  }

  return (
    <div className="flex h-full flex-col overflow-hidden bg-[#161618] text-neutral-200">
      <div className="flex-1 px-9 pt-9">
        <AnimatePresence mode="wait">
          <motion.div
            key={paso}
            initial={{ opacity: 0, x: 14 }}
            animate={{ opacity: 1, x: 0 }}
            exit={{ opacity: 0, x: -14 }}
            transition={{ duration: 0.16, ease: "easeOut" }}
            className="h-full"
          >
            {paso === 0 && <Hola ajustes={ajustes} />}
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

      <footer className="flex shrink-0 items-center justify-between border-t border-white/8 px-6 py-3">
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
                    : "w-1.5 bg-white/15"
              }`}
            />
          ))}
          <span className="ml-2 text-[11px] text-neutral-500">
            Paso {paso + 1} de {PASOS.length}
          </span>
        </span>

        <span className="flex items-center gap-2">
          {paso > 0 && (
            <button
              type="button"
              onClick={() => setPaso((p) => p - 1)}
              className="rounded-lg px-3 py-1.5 text-xs text-neutral-400 transition-colors hover:bg-white/8 hover:text-white"
            >
              Atrás
            </button>
          )}
          {paso < PASOS.length - 1 ? (
            <button
              type="button"
              onClick={() => setPaso((p) => p + 1)}
              className="rounded-lg bg-blue-600 px-4 py-1.5 text-xs font-medium text-white transition-colors hover:bg-blue-500"
            >
              {paso === 0 ? "Empezar" : "Siguiente"}
            </button>
          ) : (
            <button
              type="button"
              onClick={terminar}
              className="rounded-lg bg-blue-600 px-4 py-1.5 text-xs font-medium text-white transition-colors hover:bg-blue-500"
            >
              Todo listo
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
      <h1 className="text-[26px] leading-tight font-semibold tracking-tight text-white">{texto}</h1>
      <p className="mt-1.5 max-w-xl text-[13px] leading-relaxed text-neutral-400">{sub}</p>
    </>
  );
}

function Tecla({ children }: { children: string }) {
  return (
    <kbd className="rounded-md border border-white/12 bg-white/8 px-1.5 py-0.5 font-mono text-[11px] text-neutral-200">
      {children}
    </kbd>
  );
}

/** "CmdOrCtrl+Shift+2" no se lee: en Windows eso son las teclas Ctrl, Shift y 2. */
function partesAtajo(atajo: string): string[] {
  return atajo.split("+").map((parte) => (parte === "CmdOrCtrl" ? "Ctrl" : parte));
}

function Atajo({ valor }: { valor: string }) {
  return (
    <span className="flex items-center gap-1">
      {partesAtajo(valor).map((parte, i) => (
        <span key={parte + String(i)} className="flex items-center gap-1">
          {i > 0 && <span className="text-[10px] text-neutral-600">+</span>}
          <Tecla>{parte}</Tecla>
        </span>
      ))}
    </span>
  );
}

function Hola({ ajustes }: { ajustes: Settings }) {
  return (
    <div>
      <span className="mb-4 flex size-11 items-center justify-center rounded-2xl bg-blue-500/15 text-blue-400">
        <Sparkles className="size-5" />
      </span>
      <Titulo
        texto="winshotx ya está en marcha"
        sub="Vive en la bandeja del sistema, junto al reloj. No hay ventana que dejar abierta: se llama con una tecla, hace lo suyo y desaparece."
      />

      <div className="mt-6 grid grid-cols-2 gap-3">
        <div className="rounded-xl border border-white/8 bg-white/[0.03] p-4">
          <span className="flex items-center gap-2 text-[13px] font-medium text-neutral-200">
            <Camera className="size-4 text-neutral-500" />
            Capturar una región
          </span>
          <span className="mt-2.5 flex">
            <Atajo valor={ajustes.captureShortcut} />
          </span>
        </div>
        <div className="rounded-xl border border-white/8 bg-white/[0.03] p-4">
          <span className="flex items-center gap-2 text-[13px] font-medium text-neutral-200">
            <Video className="size-4 text-neutral-500" />
            Grabar en GIF o vídeo
          </span>
          <span className="mt-2.5 flex">
            <Atajo valor={ajustes.recordShortcut} />
          </span>
        </div>
      </div>

      <p className="mt-4 text-[12px] text-neutral-500">
        Todo se queda en tu ordenador: sin cuenta, sin nube y sin nada que subir.
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
          : "border-white/8 bg-white/[0.03] hover:border-white/20"
      }`}
    >
      <span
        aria-hidden="true"
        className={`absolute top-3.5 right-3.5 flex size-5 items-center justify-center rounded-full border transition-colors ${
          elegido ? "border-blue-500 bg-blue-500 text-white" : "border-white/20"
        }`}
      >
        {elegido && <Check className="size-3" />}
      </span>

      <span className="block text-[15px] font-semibold text-white">{titulo}</span>
      <span className="mt-0.5 block text-[12px] text-neutral-400">{resumen}</span>

      <span className="mt-3.5 mb-3.5 block space-y-2">
        {pasos.map((paso, i) => (
          <span key={paso.texto} className="flex items-center gap-2.5">
            <span className="flex size-6 shrink-0 items-center justify-center rounded-lg bg-white/8 text-neutral-300">
              {paso.icono}
            </span>
            <span className="text-[12px] text-neutral-300">
              <span className="mr-1 text-neutral-600">{i + 1}.</span>
              {paso.texto}
            </span>
          </span>
        ))}
      </span>

      <span className="mt-auto block border-t border-white/8 pt-2.5 text-[11px] text-neutral-500">
        {nota}
      </span>
    </button>
  );
}

function Estilo({ valor, onChange }: { valor: CaptureFlow; onChange: (v: CaptureFlow) => void }) {
  return (
    <div>
      <Titulo
        texto="¿Cómo prefieres capturar?"
        sub="Las dos formas usan el mismo atajo y la misma selección. Lo que cambia es lo que pasa al soltar el ratón, y se puede cambiar cuando quieras desde los ajustes."
      />

      <div className="mt-5 grid grid-cols-2 gap-3">
        <TarjetaEstilo
          elegido={valor === "toolbar"}
          onClick={() => onChange("toolbar")}
          titulo="Con barra"
          resumen="Seleccionas y eliges qué hacer."
          pasos={[
            { icono: <Keyboard className="size-3.5" />, texto: "Pulsas el atajo" },
            { icono: <MousePointer2 className="size-3.5" />, texto: "Arrastras la región" },
            {
              icono: <MousePointerClick className="size-3.5" />,
              texto: "Copiar, guardar, editar o grabar",
            },
          ]}
          nota="La opción completa: de esa barra salen el editor, el GIF y el vídeo."
        />
        <TarjetaEstilo
          elegido={valor === "instant"}
          onClick={() => onChange("instant")}
          titulo="Al vuelo"
          resumen="Seleccionas y se copia sola."
          pasos={[
            { icono: <Keyboard className="size-3.5" />, texto: "Pulsas el atajo" },
            { icono: <MousePointer2 className="size-3.5" />, texto: "Arrastras la región" },
            { icono: <Clipboard className="size-3.5" />, texto: "Ya está en el portapapeles" },
          ]}
          nota="Para pegar en un chat sin pensar. El atajo de grabar sigue sacando la barra."
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
  return (
    <div>
      <span className="mb-4 flex size-11 items-center justify-center rounded-2xl bg-blue-500/15 text-blue-400">
        <Zap className="size-5" />
      </span>
      <Titulo
        texto="¿Le quitamos la tecla a la Herramienta de Recortes?"
        sub="En Windows, la tecla Impr Pant abre la Herramienta de Recortes. Si quieres, winshotx se queda con ella y responde a ese mismo dedo, sin aprender ningún atajo nuevo."
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
              : "border-white/8 bg-white/[0.03] hover:border-white/20"
          }`}
        >
          <span className="flex items-center gap-2">
            <Tecla>Impr Pant</Tecla>
            <span aria-hidden="true" className="text-neutral-600">
              →
            </span>
            <span className="text-[13px] font-semibold text-white">winshotx</span>
          </span>
          <span className="mt-2 block text-[12px] text-neutral-400">
            Apaga el ajuste de Windows que le da la tecla a la Herramienta de Recortes y se la pasa
            a winshotx.
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
              : "border-white/8 bg-white/[0.03] hover:border-white/20"
          }`}
        >
          <span className="flex items-center gap-2">
            <Scissors className="size-4 text-neutral-500" />
            <span className="text-[13px] font-semibold text-white">Dejarla como está</span>
          </span>
          <span className="mt-2 block text-[12px] text-neutral-400">
            La Herramienta de Recortes se queda con Impr Pant y winshotx se llama con su atajo.
          </span>
        </button>
      </div>

      <div aria-live="polite" className="mt-4 min-h-[36px] text-[12px]">
        {estado?.enabled && estado.active && (
          <p className="flex items-start gap-1.5 text-emerald-400">
            <Check className="mt-0.5 size-3.5 shrink-0" />
            Hecho: Impr Pant abre winshotx. Si Windows sigue abriendo la Herramienta de Recortes,
            cierra sesión y vuelve a entrar.
          </p>
        )}
        {estado?.enabled && !estado.active && (
          <p className="text-amber-300">
            Windows no ha soltado la tecla: hay otro programa que la tiene cogida. El atajo de
            siempre sigue funcionando.
          </p>
        )}
        {estado !== null && !estado.enabled && (
          <p className="text-neutral-500">
            Sin cambios. Puedes activarlo más adelante en Ajustes, en “Atajos globales”.
          </p>
        )}
      </div>

      <p className="mt-1 text-[11px] text-neutral-600">
        Win + Mayús + S sigue siendo de Windows: esa no la puede ceder.
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
  return (
    <div>
      <span className="mb-4 flex size-11 items-center justify-center rounded-2xl bg-emerald-500/15 text-emerald-400">
        <Check className="size-5" />
      </span>
      <Titulo
        texto="Listo, ya puedes capturar"
        sub="Esto es lo que queda configurado. Todo se cambia después desde el icono de la bandeja."
      />

      <div className="mt-5 divide-y divide-white/6 overflow-hidden rounded-xl border border-white/8 bg-white/[0.03]">
        <div className="flex items-center justify-between px-4 py-2.5">
          <span className="text-[13px] text-neutral-300">Capturar una región</span>
          <span className="flex items-center gap-2">
            <Atajo valor={ajustes.captureShortcut} />
            {imprPant?.enabled && imprPant.active && (
              <>
                <span className="text-[10px] text-neutral-600">o</span>
                <Tecla>Impr Pant</Tecla>
              </>
            )}
          </span>
        </div>
        <div className="flex items-center justify-between px-4 py-2.5">
          <span className="text-[13px] text-neutral-300">Al soltar el ratón</span>
          <span className="text-[12px] text-neutral-400">
            {ajustes.captureFlow === "instant"
              ? "se copia al portapapeles"
              : "sale la barra para elegir"}
          </span>
        </div>
        <div className="flex items-center justify-between px-4 py-2.5">
          <span className="flex items-center gap-2.5">
            <Power className="size-4 text-neutral-500" />
            <span>
              <span className="block text-[13px] text-neutral-300">Arrancar con Windows</span>
              <span className="block text-[11px] text-neutral-500">
                se abre en la bandeja, sin ventana
              </span>
            </span>
          </span>
          <Switch
            checked={ajustes.startWithWindows}
            onChange={onArranque}
            label="Arrancar con Windows"
          />
        </div>
      </div>

      <p className="mt-4 text-[12px] text-neutral-500">
        Pulsa el atajo cuando quieras. Con el botón derecho en el icono de la bandeja se abren los
        ajustes.
      </p>
    </div>
  );
}
