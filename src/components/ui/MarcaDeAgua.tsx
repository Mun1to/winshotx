/**
 * El logo de winshotx, enorme y casi invisible, al fondo de la ventana.
 *
 * Es decoracion y nada mas: no se pulsa, no dice nada que no diga ya la barra de titulo, y
 * por eso va con `aria-hidden` y sin texto. La opacidad es la justa para que se note que
 * la ventana es de algo y no se note mientras se leen los ajustes; en claro hace falta un
 * pelin mas que en oscuro, porque el gris claro tapa menos.
 *
 * Va dentro del hueco de las pestañas, no de la ventana entera, y mide como mucho el 85 % de
 * su alto: desde que la ventana mide lo que mide cada pestaña (S24), en Teclas de Windows el
 * hueco se queda en 214 px y un logo de 230 asomaba por encima de la cabecera.
 */
export function MarcaDeAgua() {
  return (
    <svg
      viewBox="0 0 64 64"
      aria-hidden="true"
      className="pointer-events-none absolute right-7 bottom-5 aspect-square h-[min(230px,85%)] text-titulo opacity-[0.03] dark:opacity-[0.04]"
      fill="none"
      stroke="currentColor"
      strokeLinecap="round"
      strokeWidth="6"
    >
      {/* Las cuatro esquinas del recorte y la equis de dentro, como el icono. */}
      <path d="M8 22v-8a6 6 0 0 1 6-6h8" />
      <path d="M42 8h8a6 6 0 0 1 6 6v8" />
      <path d="M56 42v8a6 6 0 0 1-6 6h-8" />
      <path d="M22 56h-8a6 6 0 0 1-6-6v-8" />
      <path d="M24 24 40 40" />
      <path d="M40 24 24 40" />
    </svg>
  );
}
