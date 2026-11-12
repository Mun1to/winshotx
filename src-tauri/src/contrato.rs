//! Los tipos que comparten las piezas del plan de CleanShot X y Screen Studio.
//!
//! El plan (zoom a mano, fondos, webcam, cursor, edición, capturas) se hace en varias
//! sesiones a la vez, y cada una vive en sus propios archivos. Lo que pasa de una pieza a
//! otra (lo que se graba y lo que pide el editor al exportar) se fija aquí una sola vez,
//! antes de repartir, para que dos sesiones no inventen el mismo tipo por separado.
//!
//! **Nada de esto cambia lo que sale hoy.** Todo viene vacío o apagado de serie, y una
//! petición sin estos campos exporta exactamente igual que antes. Su espejo en el editor es
//! `src/lib/contrato.ts`: si cambia uno, cambia el otro en el mismo commit.
//!
//! Las posiciones van de 0 a 1 sobre la imagen grabada entera, antes de recortar, como las
//! anotaciones y la cámara de la vista previa: quien arrastra lo hace sobre una vista previa
//! que casi nunca mide lo que mide el archivo.

use serde::{Deserialize, Serialize};

use crate::encode::recorte::Recorte;

// --- Lo que se guarda al grabar (`record::SessionData`) ---

/// Una marca de la tecla de zoom, puesta mientras se graba (A3): ahí la cámara se acerca.
///
/// Va en píxeles de la región grabada, como los clics, porque sale del mismo sitio: el
/// ratón en el instante en que se pulsó la tecla.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarcaZoom {
    pub ms: u64,
    pub x: i32,
    pub y: i32,
}

/// Un rato en el que se escribió: de aquí salen acercarse al escribir (A5), esconder el
/// puntero al escribir (C2) y acelerar lo escrito (D3).
///
/// **Solo cuándo, nunca qué.** `SessionData::teclas` guarda únicamente atajos con Ctrl, Alt
/// o Win, para que una contraseña tecleada no acabe en el disco, y por eso de ahí no se
/// puede sacar cuándo se escribía. Esto guarda el rato y nada más: ni una tecla, ni cuántas.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TramoEscritura {
    pub desde_ms: u64,
    pub hasta_ms: u64,
    /// Dónde estaba el cursor de texto al empezar, en píxeles de la región grabada, si
    /// Windows lo dio. En navegadores y apps de Electron muchas veces no lo da.
    #[serde(default)]
    pub caret: Option<(i32, i32)>,
}

/// La webcam, grabada aparte como una pista más de la sesión (F2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CamaraGrabada {
    /// El nombre del archivo dentro de la carpeta de la sesión, sin la carpeta.
    pub archivo: String,
    pub ancho: u32,
    pub alto: u32,
    /// Cuándo empieza la cámara respecto al primer fotograma de la pantalla. Positivo es
    /// que la cámara arrancó más tarde.
    pub desfase_ms: i64,
}

// --- Lo que pide el editor al exportar (`exporter::ExportRequest::edicion`) ---

/// La forma de una lupa o del marco de la webcam.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Forma {
    #[default]
    Circulo,
    Cuadrado,
    Rectangulo,
    /// Rectángulo con las esquinas redondeadas.
    Redondeado,
}

/// Si la cámara de un zoom sigue al ratón o se queda quieta en su punto (A2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ModoZoom {
    /// Lo de hoy: se acerca a su punto y desde ahí acompaña al ratón, con inercia.
    #[default]
    Sigue,
    /// Anclado: el centro no se mueve en todo el tramo.
    Fijo,
}

/// Una lupa: amplía un trozo encima del vídeo sin mover la vista (A6).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Lupa {
    pub forma: Forma,
    /// Con un borde alrededor o sin él.
    pub marco: bool,
    /// El lado mayor de la lupa, de 0 a 1 sobre el ancho del vídeo exportado.
    pub tam: f32,
    /// Cuánto amplía lo de dentro: 2 es el doble.
    pub aumento: f32,
}

/// Un zoom de la pista de zooms, puesto a mano o venido de los automáticos (A1).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TramoZoom {
    pub desde_ms: u64,
    pub hasta_ms: u64,
    /// Cuánto se acerca: 2 es el doble de grande.
    pub nivel: f32,
    #[serde(default)]
    pub modo: ModoZoom,
    /// El punto, de 0 a 1. En `Sigue` es donde arranca; en `Fijo`, donde se queda.
    pub x: f32,
    pub y: f32,
    /// Entra de golpe en vez de acercarse poco a poco.
    #[serde(default)]
    pub instantaneo: bool,
    /// Con esto puesto el bloque es una lupa y no mueve la cámara.
    #[serde(default)]
    pub lupa: Option<Lupa>,
}

/// Un trozo del vídeo que se queda, y a qué velocidad (D1 y D3).
///
/// La lista va en orden y sin solaparse. Lo que cae entre dos tramos se corta, imagen y
/// sonido. La velocidad final de un tramo es la suya por la de `speed`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TramoVelocidad {
    pub desde_ms: u64,
    pub hasta_ms: u64,
    /// 1 es la de verdad, 2 el doble de rápido.
    pub velocidad: f32,
}

/// Un fondo que no es uno de los de nombre (`blanco`, `atardecer`...): B1, B2, B3 y B5.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "tipo", rename_all = "camelCase")]
pub enum FondoElegido {
    /// Una imagen del usuario, recortada para llenar.
    Imagen { ruta: String },
    /// El fondo de escritorio que tenga puesto Windows.
    Escritorio,
    /// Un color liso.
    Color { rgb: [u8; 3] },
    /// Un degradado que sale de ese color.
    Degradado { rgb: [u8; 3] },
    /// Un vídeo o un GIF ya importado, que avanza con el vídeo y se repite si es más corto.
    Video { ruta: String },
}

/// Cuánto se suaviza el puntero dibujado (C1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Suavizado {
    Nada,
    Poco,
    Mucho,
}

/// Cómo va la webcam encima del vídeo (F2, la parte del Estudio).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarcoCamara {
    pub forma: Forma,
    /// El trozo de la imagen de la cámara que se enseña, de 0 a 1.
    pub recorte: Recorte,
    /// El lado mayor, de 0 a 1 sobre el ancho del vídeo exportado.
    pub tam: f32,
    /// El centro, de 0 a 1 sobre el vídeo exportado.
    pub x: f32,
    pub y: f32,
    pub borde: bool,
}

/// Un rectángulo tapado con mosaico durante un rato (D4).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tapa {
    pub desde_ms: u64,
    pub hasta_ms: u64,
    /// La esquina de arriba a la izquierda y el tamaño, de 0 a 1.
    pub x: f32,
    pub y: f32,
    pub ancho: f32,
    pub alto: f32,
}

/// Lo nuevo que puede pedir el editor al exportar. Va dentro de `ExportRequest` aplanado:
/// en el JSON son campos sueltos de la petición, como los de siempre.
///
/// También es lo que guarda un preset con nombre (D2), sin nada de la sesión.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Edicion {
    /// Los zooms de la pista. `None` son los automáticos de hoy, sacados de los clics.
    pub zooms: Option<Vec<TramoZoom>>,
    /// Lo que se queda del vídeo y a qué velocidad. Cuando viene, manda sobre `desde_ms` y
    /// `hasta_ms`.
    pub tramos: Option<Vec<TramoVelocidad>>,
    /// Cuando viene, manda sobre `background`.
    pub fondo: Option<FondoElegido>,
    /// Cuando viene, manda sobre `smooth_cursor`.
    pub suavizado: Option<Suavizado>,
    /// Esconder el puntero mientras se escribe (C2).
    pub esconder_al_escribir: bool,
    /// El volumen del sonido de clic, de 0 a 1. 0 es sin sonido (C3).
    pub sonido_clic: f32,
    /// Una onda en cada clic, además del aro (C4).
    pub onda_clic: bool,
    /// La estela al moverse, de 0 a 1. 0 es apagada (A4).
    pub estela: f32,
    /// Acercarse a donde se escribe (A5).
    pub zoom_al_escribir: bool,
    /// Dónde y cómo va la webcam, si la sesión la trae.
    pub camara: Option<MarcoCamara>,
    /// Lo que se tapa con mosaico, y cuándo.
    pub tapar: Vec<Tapa>,
    // S10 (sonidos)
    /// Qué suena en cada clic cuando `sonido_clic` no es cero. `None` es el de fábrica, el
    /// «Ratón suave»: así suenan las peticiones y los estilos de antes de poder elegirlo.
    pub clic: Option<ClicElegido>,
}

// S10 (sonidos)
/// El sonido de cada clic (C3): uno de los tres de serie que eligió Munito, o uno propio.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "tipo", rename_all = "camelCase")]
pub enum ClicElegido {
    /// Un chasquido seco y corto, sin pitido: el de fábrica.
    Suave,
    /// Más cuerpo y más grave.
    Grave,
    /// Un solo «tic» de madera, sin golpe de soltar.
    Toque,
    /// Un archivo del usuario, ya copiado a la carpeta de la app (`sonidos::importar_sonido_de_clic`).
    Propio { ruta: String },
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Una petición de las de antes, sin ningún campo nuevo, se lee y no cambia nada.
    #[test]
    fn sin_campos_nuevos_todo_viene_apagado() {
        let edicion: Edicion = serde_json::from_str("{}").unwrap();
        assert!(edicion.zooms.is_none());
        assert!(edicion.tramos.is_none());
        assert!(edicion.fondo.is_none());
        assert!(edicion.suavizado.is_none());
        assert!(!edicion.esconder_al_escribir);
        assert_eq!(edicion.sonido_clic, 0.0);
        assert!(!edicion.onda_clic);
        assert_eq!(edicion.estela, 0.0);
        assert!(!edicion.zoom_al_escribir);
        assert!(edicion.camara.is_none());
        assert!(edicion.tapar.is_empty());
        // S10 (sonidos)
        assert!(edicion.clic.is_none());
    }

    /// Lo que escribe el editor (`src/lib/contrato.ts`) se lee tal cual.
    #[test]
    fn lee_lo_que_manda_el_editor() {
        let json = r#"{
            "zooms": [{"desdeMs": 100, "hastaMs": 900, "nivel": 2, "modo": "fijo",
                       "x": 0.25, "y": 0.5, "instantaneo": true,
                       "lupa": {"forma": "redondeado", "marco": true, "tam": 0.3, "aumento": 2.5}}],
            "tramos": [{"desdeMs": 0, "hastaMs": 500, "velocidad": 1},
                       {"desdeMs": 800, "hastaMs": 2000, "velocidad": 2}],
            "fondo": {"tipo": "degradado", "rgb": [10, 20, 30]},
            "suavizado": "mucho",
            "esconderAlEscribir": true,
            "sonidoClic": 0.3,
            "ondaClic": true,
            "estela": 0.5,
            "zoomAlEscribir": true,
            "camara": {"forma": "circulo", "recorte": {"x1": 0, "y1": 0, "x2": 1, "y2": 1},
                       "tam": 0.2, "x": 0.85, "y": 0.8, "borde": false},
            "tapar": [{"desdeMs": 0, "hastaMs": 300, "x": 0.1, "y": 0.1, "ancho": 0.2, "alto": 0.05}]
        }"#;
        let edicion: Edicion = serde_json::from_str(json).unwrap();
        let zoom = edicion.zooms.unwrap()[0];
        assert_eq!(zoom.modo, ModoZoom::Fijo);
        assert!(zoom.instantaneo);
        assert_eq!(zoom.lupa.unwrap().forma, Forma::Redondeado);
        assert_eq!(edicion.tramos.unwrap()[1].velocidad, 2.0);
        assert_eq!(edicion.fondo, Some(FondoElegido::Degradado { rgb: [10, 20, 30] }));
        assert_eq!(edicion.suavizado, Some(Suavizado::Mucho));
        assert!(edicion.esconder_al_escribir && edicion.onda_clic && edicion.zoom_al_escribir);
        assert_eq!(edicion.camara.unwrap().forma, Forma::Circulo);
        assert_eq!(edicion.tapar.len(), 1);
    }

    /// Los fondos sin nada dentro también se leen, y los de ruta guardan su ruta.
    #[test]
    fn los_fondos_se_leen_por_su_tipo() {
        let escritorio: FondoElegido = serde_json::from_str(r#"{"tipo": "escritorio"}"#).unwrap();
        assert_eq!(escritorio, FondoElegido::Escritorio);
        let video: FondoElegido =
            serde_json::from_str(r#"{"tipo": "video", "ruta": "C:\\fondo.mp4"}"#).unwrap();
        assert_eq!(video, FondoElegido::Video { ruta: "C:\\fondo.mp4".into() });
    }

    // S10 (sonidos)
    /// El sonido de clic se lee por su tipo, como los fondos, y el propio guarda su ruta.
    #[test]
    fn el_sonido_de_clic_se_lee_por_su_tipo() {
        let edicion: Edicion = serde_json::from_str(r#"{"sonidoClic": 0.3, "clic": {"tipo": "grave"}}"#).unwrap();
        assert_eq!(edicion.clic, Some(ClicElegido::Grave));
        let propio: ClicElegido = serde_json::from_str(r#"{"tipo": "propio", "ruta": "C:\\clic.wav"}"#).unwrap();
        assert_eq!(propio, ClicElegido::Propio { ruta: "C:\\clic.wav".into() });
        let nulo: Edicion = serde_json::from_str(r#"{"clic": null}"#).unwrap();
        assert!(nulo.clic.is_none());
    }
}
