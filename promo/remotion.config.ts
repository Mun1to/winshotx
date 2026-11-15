import { Config } from "@remotion/cli/config";

/* H.264 en MP4: lo que reproducen Reddit, X y GitHub sin recodificar. */
Config.setVideoImageFormat("jpeg");
Config.setCodec("h264");
Config.setOverwriteOutput(true);
/* La gráfica por ANGLE, como en Adeorq/promo: con la que elige el navegador, en esta
   máquina se quedaba sin contexto de WebGL a mitad de render. */
Config.setChromiumOpenGlRenderer("angle");
/* Un render a la vez y con pocos hilos: el PC de Munito va cargado (el encargo lo pide). */
Config.setConcurrency(2);
