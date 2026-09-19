import { motion, useReducedMotion } from "framer-motion";
import type { ReactNode } from "react";

interface Props {
  children: ReactNode;
  className?: string;
  /** Aparicion tipo macOS: escala corta y subida de 4px. */
  animate?: boolean;
}

export function GlassPanel({ children, className = "", animate = true }: Props) {
  // Quien pide menos movimiento no ve el panel escalar, y ademas aparece de golpe en vez
  // de subir de opacidad: una barra que se desvanece hacia dentro tampoco es "menos
  // movimiento" si lo que quieres es que este ahi cuando miras.
  const quieto = useReducedMotion();
  const base =
    "rounded-2xl border border-linea-fuerte bg-flotante/90 backdrop-blur-xl shadow-2xl";
  if (!animate || quieto) return <div className={`${base} ${className}`}>{children}</div>;
  return (
    <motion.div
      initial={{ opacity: 0, scale: 0.92, y: 4 }}
      animate={{ opacity: 1, scale: 1, y: 0 }}
      exit={{ opacity: 0, scale: 0.96, y: 2 }}
      transition={{ type: "spring", stiffness: 520, damping: 34, mass: 0.6 }}
      className={`${base} ${className}`}
    >
      {children}
    </motion.div>
  );
}
