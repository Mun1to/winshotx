"""
Dibuja las dos imágenes de 1200x630 que salen al pegar el enlace en X, Discord o Slack.

    python frontlaxweb/generar-social.py

Necesita Pillow y las fuentes de Windows. El logo se dibuja aquí mismo, así que no depende
de ningún archivo suelto.
"""
import math
import pathlib
from PIL import Image, ImageDraw, ImageFont

AQUI = pathlib.Path(__file__).parent
AZUL = (10, 155, 255)
FONDO = (11, 11, 13)


def arco(cx, cy, r, a0, a1, n=18):
    return [(cx + r * math.cos(math.radians(a0 + (a1 - a0) * i / n)),
             cy + r * math.sin(math.radians(a0 + (a1 - a0) * i / n))) for i in range(n + 1)]


def logo(tam):
    """El mismo trazo del icono: cuatro esquinas gruesas y una X blanca."""
    ss, k = 4, tam / 64
    img = Image.new("RGBA", (tam * ss, tam * ss), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)

    def trazo(pts, w, color):
        p = [(x * k * ss, y * k * ss) for x, y in pts]
        r = w * k * ss / 2
        for (x0, y0), (x1, y1) in zip(p, p[1:]):
            pasos = max(1, int(math.hypot(x1 - x0, y1 - y0)))
            for i in range(pasos + 1):
                t = i / pasos
                x, y = x0 + (x1 - x0) * t, y0 + (y1 - y0) * t
                d.ellipse([x - r, y - r, x + r, y + r], fill=color)

    R = 6
    trazo([(8, 22)] + arco(8 + R, 14, R, 180, 270) + [(22, 8)], 8, AZUL + (255,))
    trazo([(42, 8)] + arco(56 - R, 14, R, 270, 360) + [(56, 22)], 8, AZUL + (255,))
    trazo([(56, 42)] + arco(56 - R, 50, R, 0, 90) + [(42, 56)], 8, AZUL + (255,))
    trazo([(22, 56)] + arco(8 + R, 50, R, 90, 180) + [(8, 42)], 8, AZUL + (255,))
    trazo([(24, 24), (40, 40)], 8.5, (255, 255, 255, 255))
    trazo([(40, 24), (24, 40)], 8.5, (255, 255, 255, 255))
    return img.resize((tam, tam), Image.LANCZOS)


# El PNG de la marca sale de la misma funcion que el logo de las tarjetas. Antes se
# guardaba a mano y se quedo con la cruz vieja cuando el trazo paso a ser una X: si solo
# hay una fuente, no se pueden desincronizar.
marca_png = logo(1024)
marca_png.save(AQUI.parent / "docs" / "img" / "logo.png")
print("hecha: docs/img/logo.png")


def fuente(nombre, tam):
    return ImageFont.truetype(rf"C:\Windows\Fonts\{nombre}", tam)


def tarjeta(destino, titulo1, titulo2, bajada, datos):
    W, H = 1200, 630
    img = Image.new("RGBA", (W, H), FONDO + (255,))
    halo = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    hd = ImageDraw.Draw(halo)
    for r in range(520, 0, -8):
        hd.ellipse([200 - r, 150 - r, 200 + r, 150 + r],
                   fill=AZUL + (int(26 * (1 - r / 520) ** 1.6),))
    img = Image.alpha_composite(img, halo).convert("RGB")
    d = ImageDraw.Draw(img)

    marca = logo(96)
    img.paste(marca, (72, 62), marca)
    d.text((188, 78), "winshotx", font=fuente("segoeuib.ttf", 52), fill=(255, 255, 255))

    d.text((72, 208), titulo1, font=fuente("segoeuib.ttf", 76), fill=(255, 255, 255))
    d.text((72, 292), titulo2, font=fuente("segoeuib.ttf", 76), fill=AZUL)
    d.text((72, 404), bajada, font=fuente("segoeui.ttf", 28), fill=(142, 142, 154))

    x = 72
    for i, (valor, que, contra) in enumerate(datos):
        if i:
            d.line([(x - 30, 486), (x - 30, 566)], fill=(40, 40, 46), width=1)
        d.text((x, 480), valor, font=fuente("segoeuib.ttf", 46), fill=AZUL)
        d.text((x, 534), que, font=fuente("segoeui.ttf", 24), fill=(233, 233, 238))
        d.text((x, 562), contra, font=fuente("segoeui.ttf", 20), fill=(110, 110, 120))
        x += 270

    img.save(AQUI / destino)
    print("hecha:", destino)


tarjeta(
    "social.png",
    "Recorta la pantalla", "antes de que parpadees",
    "Captura y grabación de pantalla para Windows · 2,2 MB · código abierto",
    [("28 ms", "en abrir", "Recortes: 920 ms"),
     ("33 MB", "capturando", "Recortes: 253 MB"),
     ("31 MB", "en reposo", "Recortes: 98 MB"),
     ("2,2 MB", "instalador", "sin dependencias")],
)

tarjeta(
    "social-en.png",
    "Crop the screen", "before you blink",
    "Screenshots and screen recording for Windows · 2.2 MB · open source",
    [("28 ms", "to open", "Snipping Tool: 920 ms"),
     ("33 MB", "capturing", "Snipping Tool: 253 MB"),
     ("31 MB", "idle", "Snipping Tool: 98 MB"),
     ("2.2 MB", "installer", "nothing bundled")],
)
