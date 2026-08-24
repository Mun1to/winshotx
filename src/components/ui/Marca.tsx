/**
 * El logo de winshotx: las cuatro esquinas de un recorte y la cruz del centro.
 * Es el mismo dibujo que el icono de la bandeja y el de la web, para que quien instala
 * la app reconozca lo que vio antes.
 */
export function Marca({ className = "" }: { className?: string }) {
  return (
    <svg viewBox="0 0 64 64" role="img" aria-label="winshotx" className={className}>
      <g fill="none" stroke="#0a9bff" strokeLinecap="round">
        <g strokeWidth="8">
          <path d="M8 22v-8a6 6 0 0 1 6-6h8" />
          <path d="M42 8h8a6 6 0 0 1 6 6v8" />
          <path d="M56 42v8a6 6 0 0 1-6 6h-8" />
          <path d="M22 56h-8a6 6 0 0 1-6-6v-8" />
        </g>
        <g strokeWidth="3.5">
          <path d="M24 8h16" />
          <path d="M56 24v16" />
          <path d="M40 56H24" />
          <path d="M8 40V24" />
        </g>
      </g>
      <g stroke="#fff" strokeWidth="8.5" strokeLinecap="round">
        <path d="M32 20.5v23" />
        <path d="M20.5 32h23" />
      </g>
    </svg>
  );
}
