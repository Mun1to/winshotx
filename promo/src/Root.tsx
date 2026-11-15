import { Composition } from "remotion";
import { ALTO, ANCHO, FPS } from "./casa";
import { D1, DireccionCarrera } from "./direcciones/D1Carrera";
import { D2, DireccionTirada } from "./direcciones/D2Tirada";
import { D3, DireccionLoHiceYo } from "./direcciones/D3LoHiceYo";
import { Promo, T } from "./Promo";

/**
 * Las tres direcciones del vídeo de la 0.5, para que Munito elija una. Cada una con su guion
 * entero por tiempos; de cada una se renderiza el arranque y unas fotos (`render.mjs`), no el
 * vídeo entero: el entero se monta cuando elija.
 */
export function Root() {
  return (
    <>
      <Composition id="Promo" component={Promo} durationInFrames={T.fin} fps={FPS} width={ANCHO} height={ALTO} />
      <Composition id="Direccion1-Carrera" component={DireccionCarrera} durationInFrames={D1.fin} fps={FPS} width={ANCHO} height={ALTO} />
      <Composition id="Direccion2-Tirada" component={DireccionTirada} durationInFrames={D2.fin} fps={FPS} width={ANCHO} height={ALTO} />
      <Composition id="Direccion3-LoHiceYo" component={DireccionLoHiceYo} durationInFrames={D3.fin} fps={FPS} width={ANCHO} height={ALTO} />
    </>
  );
}
