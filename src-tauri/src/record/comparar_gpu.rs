//! Que ha cambiado entre dos fotogramas, preguntado a la tarjeta grafica.
//!
//! **Por que existe (S29).** Grabando la pantalla, casi siempre cambia solo el raton, y hasta
//! la 0.4.8 cada fotograma se traia ENTERO de la GPU (ocho megabytes a 1920x1080) para
//! compararlo entero en la CPU y descubrir eso. Con el anillo de los ultimos segundos a
//! 60 fps eran el 57 % de un nucleo en la maquina de Munito, toda la tarde.
//!
//! Windows 11 dice que zonas cambian (`DirtyRegionMode`), y fue lo primero que se probo:
//! **no vale**. Medido con `verificar_las_zonas_sin_pantalla`, en 71 de 256 fotogramas con
//! un cuadrado moviendose habia pixeles cambiados fuera de las zonas que dijo (cuando el
//! fotograma anterior aun no se habia soltado, se callaba lo que cambio en medio). Ademas
//! solo existe desde Windows 11 24H2.
//!
//! Asi que se compara en la GPU, que es donde ya esta la imagen: se copia el fotograma a una
//! textura propia (una copia de GPU a GPU, que no le cuesta nada al procesador), un
//! sombreador compara cada pixel con el fotograma anterior y deja un 1 por cada tesela de
//! 16x16 que tenga algo distinto, y lo unico que viaja a la CPU es esa mascara (32 KB a
//! 1920x1080) y despues los pixeles de las teselas marcadas. Es exacto por construccion:
//! compara los pixeles de verdad, no lo que alguien dice que cambio. Y funciona en cualquier
//! Windows en el que funcione la captura (nivel de funciones 11, que el crate ya exige).
//!
//! El sombreador esta en `comparar.hlsl` y va compilado dentro (`comparar.cso`).

use windows::Win32::Graphics::Direct3D11::{
    ID3D11Buffer, ID3D11ComputeShader, ID3D11Device, ID3D11DeviceContext, ID3D11ShaderResourceView,
    ID3D11Texture2D, ID3D11UnorderedAccessView, D3D11_BIND_SHADER_RESOURCE, D3D11_BIND_UNORDERED_ACCESS,
    D3D11_BOX, D3D11_BUFFER_DESC, D3D11_BUFFER_UAV, D3D11_BUFFER_UAV_FLAG_RAW,
    D3D11_CPU_ACCESS_READ, D3D11_MAPPED_SUBRESOURCE, D3D11_MAP_READ, D3D11_RESOURCE_MISC_BUFFER_ALLOW_RAW_VIEWS,
    D3D11_TEXTURE2D_DESC, D3D11_UAV_DIMENSION_BUFFER, D3D11_UNORDERED_ACCESS_VIEW_DESC,
    D3D11_UNORDERED_ACCESS_VIEW_DESC_0, D3D11_USAGE_DEFAULT, D3D11_USAGE_STAGING,
};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT, DXGI_FORMAT_R32_TYPELESS, DXGI_SAMPLE_DESC};

use crate::record::delta::Parche;

/// El sombreador ya compilado. Ver `comparar.hlsl` para rehacerlo.
const SOMBREADOR: &[u8] = include_bytes!("comparar.cso");

/// Lado de una tesela, en pixeles. Tiene que ser el `numthreads` del sombreador.
pub const TESELA: u32 = 16;

/// Si cambia mas de esta parte de las teselas, se lee entero: muchas copias pequennas a la
/// GPU cuestan mas que una grande, y el trabajo de juntar zonas tambien.
const PARTE_PARA_ENTERO: f64 = 0.4;

type R<T> = std::result::Result<T, windows::core::Error>;

fn fallo() -> windows::core::Error {
    windows::core::Error::from(windows::Win32::Foundation::E_FAIL)
}

/// Las dos texturas que se turnan (la del fotograma anterior y la del nuevo), la mascara y el
/// sombreador. Mide lo que mide la region que se graba.
pub struct Comparador {
    ancho: u32,
    alto: u32,
    texturas: [ID3D11Texture2D; 2],
    vistas: [ID3D11ShaderResourceView; 2],
    /// Cual de las dos tiene el ultimo fotograma comparado. `None` hasta el primero.
    anterior: Option<usize>,
    mascara: ID3D11Buffer,
    mascara_uav: ID3D11UnorderedAccessView,
    mascara_lectura: ID3D11Buffer,
    sombreador: ID3D11ComputeShader,
    teselas: (u32, u32),
}

/// Lo que ha salido de comparar un fotograma con el anterior.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cambio {
    /// Es el primero, o ha cambiado tanto que se lee entero.
    Entero,
    /// Nada distinto: el mismo fotograma.
    Nada,
    /// Estas zonas (en coordenadas de la region), ya juntas.
    Zonas(Vec<Parche>),
}

impl Comparador {
    pub fn nuevo(device: &ID3D11Device, formato: DXGI_FORMAT, ancho: u32, alto: u32) -> R<Self> {
        let textura = || -> R<ID3D11Texture2D> {
            let desc = D3D11_TEXTURE2D_DESC {
                Width: ancho,
                Height: alto,
                MipLevels: 1,
                ArraySize: 1,
                Format: formato,
                SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: D3D11_BIND_SHADER_RESOURCE.0 as u32,
                CPUAccessFlags: 0,
                MiscFlags: 0,
            };
            let mut t = None;
            unsafe { device.CreateTexture2D(&desc, None, Some(&mut t))? };
            t.ok_or_else(fallo)
        };
        let vista = |t: &ID3D11Texture2D| -> R<ID3D11ShaderResourceView> {
            let mut v = None;
            unsafe { device.CreateShaderResourceView(t, None, Some(&mut v))? };
            v.ok_or_else(fallo)
        };
        let texturas = [textura()?, textura()?];
        let vistas = [vista(&texturas[0])?, vista(&texturas[1])?];

        let teselas = (ancho.div_ceil(TESELA), alto.div_ceil(TESELA));
        let bytes = teselas.0 * teselas.1 * 4;
        let mut mascara = None;
        unsafe {
            device.CreateBuffer(
                &D3D11_BUFFER_DESC {
                    ByteWidth: bytes,
                    Usage: D3D11_USAGE_DEFAULT,
                    BindFlags: D3D11_BIND_UNORDERED_ACCESS.0 as u32,
                    CPUAccessFlags: 0,
                    MiscFlags: D3D11_RESOURCE_MISC_BUFFER_ALLOW_RAW_VIEWS.0 as u32,
                    StructureByteStride: 0,
                },
                None,
                Some(&mut mascara),
            )?
        };
        let mascara: ID3D11Buffer = mascara.ok_or_else(fallo)?;
        let mut mascara_uav = None;
        unsafe {
            device.CreateUnorderedAccessView(
                &mascara,
                Some(&D3D11_UNORDERED_ACCESS_VIEW_DESC {
                    Format: DXGI_FORMAT_R32_TYPELESS,
                    ViewDimension: D3D11_UAV_DIMENSION_BUFFER,
                    Anonymous: D3D11_UNORDERED_ACCESS_VIEW_DESC_0 {
                        Buffer: D3D11_BUFFER_UAV {
                            FirstElement: 0,
                            NumElements: teselas.0 * teselas.1,
                            Flags: D3D11_BUFFER_UAV_FLAG_RAW.0 as u32,
                        },
                    },
                }),
                Some(&mut mascara_uav),
            )?
        };
        let mut mascara_lectura = None;
        unsafe {
            device.CreateBuffer(
                &D3D11_BUFFER_DESC {
                    ByteWidth: bytes,
                    Usage: D3D11_USAGE_STAGING,
                    BindFlags: 0,
                    CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
                    MiscFlags: 0,
                    StructureByteStride: 0,
                },
                None,
                Some(&mut mascara_lectura),
            )?
        };
        let mut sombreador = None;
        unsafe { device.CreateComputeShader(SOMBREADOR, None, Some(&mut sombreador))? };
        Ok(Self {
            ancho,
            alto,
            texturas,
            vistas,
            anterior: None,
            mascara,
            mascara_uav: mascara_uav.ok_or_else(fallo)?,
            mascara_lectura: mascara_lectura.ok_or_else(fallo)?,
            sombreador: sombreador.ok_or_else(fallo)?,
            teselas,
        })
    }

    pub fn medida(&self) -> (u32, u32) {
        (self.ancho, self.alto)
    }

    /// La textura con el ultimo fotograma comparado (la region ya recortada), para leer de
    /// ella. `None` antes del primero.
    pub fn ultimo(&self) -> Option<&ID3D11Texture2D> {
        self.anterior.map(|i| &self.texturas[i])
    }

    /// Que cuando la siguiente vez no haya con que comparar: el siguiente sale `Entero`.
    pub fn olvidar(&mut self) {
        self.anterior = None;
    }

    /// Copia la region (`origen` empieza en `esquina`) del fotograma y la compara con la
    /// anterior. Despues de esto, `ultimo()` es este fotograma.
    pub fn comparar(&mut self, contexto: &ID3D11DeviceContext, fotograma: &ID3D11Texture2D, esquina: (u32, u32)) -> R<Cambio> {
        let nueva = self.anterior.map_or(0, |i| 1 - i);
        let caja = D3D11_BOX {
            left: esquina.0,
            top: esquina.1,
            front: 0,
            right: esquina.0 + self.ancho,
            bottom: esquina.1 + self.alto,
            back: 1,
        };
        unsafe {
            contexto.CopySubresourceRegion(&self.texturas[nueva], 0, 0, 0, 0, fotograma, 0, Some(&caja));
        }
        let Some(anterior) = self.anterior.replace(nueva) else {
            return Ok(Cambio::Entero);
        };

        unsafe {
            contexto.CSSetShader(&self.sombreador, None);
            contexto.CSSetShaderResources(0, Some(&[Some(self.vistas[anterior].clone()), Some(self.vistas[nueva].clone())]));
            contexto.CSSetUnorderedAccessViews(0, 1, Some(&Some(self.mascara_uav.clone())), None);
            contexto.Dispatch(self.teselas.0, self.teselas.1, 1);
            // Se suelta todo: la textura de ahora sera la de leer en la siguiente vuelta, y
            // una textura atada a la vez para escribir y para leer la desata Direct3D sola.
            contexto.CSSetShaderResources(0, Some(&[None, None]));
            contexto.CSSetUnorderedAccessViews(0, 1, Some(&None), None);
            contexto.CSSetShader(None, None);
            contexto.CopyResource(&self.mascara_lectura, &self.mascara);
        }
        let mut mapa = D3D11_MAPPED_SUBRESOURCE::default();
        unsafe { contexto.Map(&self.mascara_lectura, 0, D3D11_MAP_READ, 0, Some(&mut mapa))? };
        let n = (self.teselas.0 * self.teselas.1) as usize;
        // SAFETY: el bufer mide `n` u32 y esta mapeado hasta el Unmap de abajo.
        let marcas = unsafe { std::slice::from_raw_parts(mapa.pData as *const u32, n) };
        let cambio = zonas_de_la_mascara(marcas, self.teselas, (self.ancho, self.alto));
        unsafe { contexto.Unmap(&self.mascara_lectura, 0) };
        Ok(cambio)
    }
}

/// De la mascara de teselas a rectangulos: tramos de teselas seguidas en cada fila, que se
/// alargan hacia abajo mientras la fila siguiente tenga uno que los toque, y se juntan hasta
/// quedarse en pocos (`delta::juntar_zonas`).
pub fn zonas_de_la_mascara(marcas: &[u32], teselas: (u32, u32), medida: (u32, u32)) -> Cambio {
    let (tw, th) = teselas;
    let marcadas = marcas.iter().filter(|&&m| m != 0).count();
    if marcadas == 0 {
        return Cambio::Nada;
    }
    if marcadas as f64 > f64::from(tw * th) * PARTE_PARA_ENTERO {
        return Cambio::Entero;
    }
    // Cajas en teselas: (x1, y1, x2, y2), x2/y2 exclusivos. Las de la fila anterior siguen
    // «abiertas» mientras algo de la fila siguiente las toque.
    let mut cerradas: Vec<(u32, u32, u32, u32)> = Vec::new();
    let mut abiertas: Vec<(u32, u32, u32, u32)> = Vec::new();
    for y in 0..th {
        let fila = &marcas[(y * tw) as usize..((y + 1) * tw) as usize];
        let mut tramos = Vec::new();
        let mut x = 0;
        while x < tw {
            if fila[x as usize] != 0 {
                let desde = x;
                while x < tw && fila[x as usize] != 0 {
                    x += 1;
                }
                tramos.push((desde, x));
            } else {
                x += 1;
            }
        }
        let mut siguen = Vec::new();
        for caja in abiertas.drain(..) {
            let tocada = tramos.iter().any(|&(a, b)| a <= caja.2 && caja.0 <= b);
            if tocada {
                siguen.push(caja);
            } else {
                cerradas.push(caja);
            }
        }
        for (a, b) in tramos {
            if let Some(caja) = siguen.iter_mut().find(|c| a <= c.2 && c.0 <= b) {
                caja.0 = caja.0.min(a);
                caja.2 = caja.2.max(b);
                caja.3 = y + 1;
            } else {
                siguen.push((a, y, b, y + 1));
            }
        }
        abiertas = siguen;
    }
    cerradas.extend(abiertas);
    let (ancho, alto) = medida;
    let mut zonas: Vec<Parche> = cerradas
        .into_iter()
        .map(|(x1, y1, x2, y2)| {
            let (x, y) = (x1 * TESELA, y1 * TESELA);
            Parche {
                x,
                y,
                width: (x2 * TESELA).min(ancho) - x,
                height: (y2 * TESELA).min(alto) - y,
            }
        })
        .collect();
    crate::record::delta::juntar_zonas(&mut zonas, 8);
    Cambio::Zonas(zonas)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mascara(tw: u32, th: u32, marcadas: &[(u32, u32)]) -> Vec<u32> {
        let mut m = vec![0u32; (tw * th) as usize];
        for &(x, y) in marcadas {
            m[(y * tw + x) as usize] = 1;
        }
        m
    }

    #[test]
    fn sin_marcas_no_ha_cambiado_nada() {
        assert_eq!(zonas_de_la_mascara(&mascara(120, 68, &[]), (120, 68), (1920, 1080)), Cambio::Nada);
    }

    /// Cada tesela marcada queda dentro de alguna zona, y las del borde no se salen de la
    /// pantalla (1080 no es multiplo de 16).
    #[test]
    fn las_teselas_marcadas_quedan_cubiertas() {
        let marcadas = [(0, 0), (1, 0), (1, 1), (50, 30), (119, 67), (60, 67), (61, 66)];
        let Cambio::Zonas(zonas) = zonas_de_la_mascara(&mascara(120, 68, &marcadas), (120, 68), (1920, 1080)) else {
            panic!("tenian que salir zonas");
        };
        assert!(zonas.len() <= 8);
        for (tx, ty) in marcadas {
            let (x, y) = (tx * TESELA, ty * TESELA);
            assert!(
                zonas.iter().any(|z| z.x <= x && x < z.x + z.width && z.y <= y && y < z.y + z.height),
                "la tesela {tx},{ty} se ha quedado fuera de {zonas:?}"
            );
        }
        for z in &zonas {
            assert!(z.x + z.width <= 1920 && z.y + z.height <= 1080, "{z:?} se sale");
        }
    }

    #[test]
    fn si_cambia_casi_todo_se_lee_entero() {
        let todas: Vec<(u32, u32)> = (0..68).flat_map(|y| (0..120).map(move |x| (x, y))).collect();
        assert_eq!(zonas_de_la_mascara(&mascara(120, 68, &todas), (120, 68), (1920, 1080)), Cambio::Entero);
    }

    /// Una forma en U: los dos brazos se juntan abajo, y nada se queda sin cubrir.
    #[test]
    fn una_u_se_cubre_entera() {
        let mut marcadas = Vec::new();
        for y in 10..20 {
            marcadas.push((5, y));
            marcadas.push((15, y));
        }
        for x in 5..16 {
            marcadas.push((x, 20));
        }
        let Cambio::Zonas(zonas) = zonas_de_la_mascara(&mascara(40, 30, &marcadas), (40, 30), (640, 480)) else {
            panic!("tenian que salir zonas");
        };
        for (tx, ty) in marcadas {
            let (x, y) = (tx * TESELA, ty * TESELA);
            assert!(zonas.iter().any(|z| z.x <= x && x < z.x + z.width && z.y <= y && y < z.y + z.height));
        }
    }
}
