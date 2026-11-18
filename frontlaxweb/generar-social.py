"""
Dibuja la imagen de 1200x630 que sale al pegar el enlace en X, Discord o Slack (social.png y
social-en.png, las dos en inglés), con la ventana de Ajustes de la app de verdad dentro.

    python frontlaxweb/generar-social.py

Necesita Pillow y las fuentes de Windows. El logo se dibuja aquí mismo, así que no depende
de ningún archivo suelto.
"""
import math
import pathlib
import subprocess
import tempfile
from PIL import Image, ImageDraw, ImageFilter, ImageFont

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


def captura_de_la_app():
    """Los Ajustes de la app de verdad, en inglés y en oscuro, a 2x: la saca el banco de fotos
    (`scripts/ver-ventana.mjs`, sobre `dist/`, así que antes hace falta `pnpm build`). Así la
    tarjeta enseña siempre la versión de hoy y nunca una foto vieja guardada a mano."""
    destino = pathlib.Path(tempfile.gettempdir()) / "winshotx-tarjeta-ajustes.png"
    subprocess.run(["node", str(AQUI.parent / "scripts" / "ver-ventana.mjs"), str(destino),
                    "--seccion=grabar", "--idioma=en", "--tema=oscuro", "--escala=2"],
                   check=True, cwd=AQUI.parent, stdout=subprocess.DEVNULL)
    return Image.open(destino).convert("RGB")


def esquinas(img, radio):
    mascara = Image.new("L", img.size, 0)
    ImageDraw.Draw(mascara).rounded_rectangle([0, 0, img.size[0] - 1, img.size[1] - 1], radio, fill=255)
    return mascara


def tarjeta(destinos, titulo1, titulo2, bajada, datos, app):
    """La de 1200x630 que sale al pegar el enlace. En inglés en las dos portadas: Munito, 15-nov,
    con la de antes delante («hay que arreglar y mejorar la foto, y que esté en inglés»)."""
    W, H = 1200, 630
    img = Image.new("RGBA", (W, H), FONDO + (255,))
    halo = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    hd = ImageDraw.Draw(halo)
    for cx, cy, R, a in ((930, 330, 560, 34), (120, 90, 420, 18)):
        for r in range(R, 0, -8):
            hd.ellipse([cx - r, cy - r, cx + r, cy + r], fill=AZUL + (int(a * (1 - r / R) ** 1.7),))
    img = Image.alpha_composite(img, halo)

    # La ventana de la app, grande y saliéndose por la derecha y por abajo, con su sombra.
    ancho = 660
    foto = app.resize((ancho, round(app.size[1] * ancho / app.size[0])), Image.LANCZOS)
    x0, y0 = 612, 84
    sombra = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    ImageDraw.Draw(sombra).rounded_rectangle([x0 + 6, y0 + 22, x0 + foto.size[0] + 6, y0 + foto.size[1] + 22], 18,
                                             fill=(0, 0, 0, 170))
    img = Image.alpha_composite(img, sombra.filter(ImageFilter.GaussianBlur(26)))
    img.paste(foto, (x0, y0), esquinas(foto, 18))
    d = ImageDraw.Draw(img)
    d.rounded_rectangle([x0, y0, x0 + foto.size[0] - 1, y0 + foto.size[1] - 1], 18, outline=(58, 58, 66), width=2)

    # Lo de la izquierda, con un velo por si la foto llega debajo.
    marca = logo(80)
    img.paste(marca, (64, 60), marca)
    d.text((160, 70), "winshotx", font=fuente("segoeuib.ttf", 46), fill=(255, 255, 255))

    # El titular, al tamaño mayor que quepa antes de la ventana (medido, no a ojo).
    hueco = x0 - 64 - 40
    tam = 66
    while tam > 40 and max(d.textlength(t, font=fuente("segoeuib.ttf", tam)) for t in (titulo1, titulo2)) > hueco:
        tam -= 2
    grande = fuente("segoeuib.ttf", tam)
    d.text((62, 190), titulo1, font=grande, fill=(255, 255, 255))
    d.text((62, 190 + tam * 1.12), titulo2, font=grande, fill=AZUL)
    d.text((64, 190 + tam * 2.45), bajada, font=fuente("segoeui.ttf", 26), fill=(170, 170, 182))

    # Las cifras, cada una a su ancho y con su raya en medio.
    fv, fq = fuente("segoeuib.ttf", 40), fuente("segoeui.ttf", 23)
    x = 64
    for i, (valor, que) in enumerate(datos):
        if i:
            d.line([(x - 22, 464), (x - 22, 546)], fill=(52, 52, 60), width=2)
        d.text((x, 456), valor, font=fv, fill=AZUL)
        d.text((x, 512), que, font=fq, fill=(225, 225, 232))
        x += max(d.textlength(valor, font=fv), d.textlength(que, font=fq)) + 44
    assert x - 44 <= x0 - 30, f"las cifras no caben: llegan a {x - 44}"

    final = img.convert("RGB")
    for destino in destinos:
        final.save(AQUI / destino, optimize=True)
        print("hecha:", destino)


tarjeta(
    ["social.png", "social-en.png"],
    "Crop the screen", "before you blink",
    "Screenshots, videos and GIFs for Windows",
    [("114 ms", "to show it"),
     ("3.01 MB", "installer"),
     ("Free", "open source")],
    captura_de_la_app(),
)


def miniatura_del_video(destino, rotulo):
    """La misma tarjeta con un botón de reproducir encima de la ventana: es la portada del vídeo en
    el README. Sale de la tarjeta para que las cifras de las dos no puedan contar cosas distintas."""
    img = Image.open(AQUI / "social-en.png").convert("RGBA")
    W, H = img.size
    # La ventana, desenfocada y más oscura: si no, sus textos se cruzan con el rótulo del botón.
    ventana = Image.new("L", (W, H), 0)
    ImageDraw.Draw(ventana).rounded_rectangle([612, 84, W + 40, H + 40], 18, fill=255)
    img.paste(img.filter(ImageFilter.GaussianBlur(1.6)), (0, 0), ventana)
    velo = Image.new("RGBA", (W, H), (6, 8, 12, 0))
    velo.putalpha(ventana.point(lambda v: 125 if v else 0))
    img = Image.alpha_composite(img, velo)

    cx, cy, r, ss = 906, 318, 68, 4
    sombra = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    ImageDraw.Draw(sombra).ellipse([cx - r - 4, cy - r + 8, cx + r + 4, cy + r + 16], fill=(0, 0, 0, 190))
    img = Image.alpha_composite(img, sombra.filter(ImageFilter.GaussianBlur(18)))

    boton = Image.new("RGBA", (2 * r * ss, 2 * r * ss), (0, 0, 0, 0))
    d = ImageDraw.Draw(boton)
    d.ellipse([0, 0, 2 * r * ss - 1, 2 * r * ss - 1], fill=AZUL + (255,))
    # El triángulo va un poco a la derecha: centrado por su caja parece caído hacia la izquierda.
    c, t = r * ss, 28 * ss
    d.polygon([(c - t * 0.62, c - t), (c - t * 0.62, c + t), (c + t * 1.08, c)], fill=(255, 255, 255, 255))
    boton = boton.resize((2 * r, 2 * r), Image.LANCZOS)
    img.paste(boton, (cx - r, cy - r), boton)

    d = ImageDraw.Draw(img)
    letra = fuente("segoeuib.ttf", 30)
    d.text((cx - d.textlength(rotulo, font=letra) / 2, cy + r + 22), rotulo, font=letra, fill=(255, 255, 255))
    img.convert("RGB").save(AQUI.parent / ".github" / "readme" / destino, optimize=True)
    print("hecha:", destino)


miniatura_del_video("video-en.png", "Watch the video · 45 s")
miniatura_del_video("video-es.png", "Mira el vídeo · 45 s")
