
import { useT } from "../../lib/i18n";/**
 * El logo de winshotx: las cuatro esquinas de un recorte y la X del centro.
 * Es el mismo dibujo que el icono de la bandeja y el de la web, para que quien instala
 * la app reconozca lo que vio antes. La X va en blanco porque aqui el fondo siempre es
 * oscuro; el icono de la app la lleva en azul, que es el que cae en fondos ajenos.
 */
export function Marca({ className = "" }: { className?: string }) {
  const t = useT();
  return (
    <svg viewBox="0 0 64 64" role="img" aria-label={t("winshotx")} className={className}>
      <g fill="none" stroke="#0a9bff" strokeLinecap="round" strokeWidth="8">
        <path d="M8 22v-8a6 6 0 0 1 6-6h8" />
        <path d="M42 8h8a6 6 0 0 1 6 6v8" />
        <path d="M56 42v8a6 6 0 0 1-6 6h-8" />
        <path d="M22 56h-8a6 6 0 0 1-6-6v-8" />
      </g>
      <g stroke="#fff" strokeWidth="8.5" strokeLinecap="round">
        <path d="M24 24 40 40" />
        <path d="M40 24 24 40" />
      </g>
    </svg>
  );
}
