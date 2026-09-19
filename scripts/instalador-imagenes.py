#!/usr/bin/env python3
"""NodeFlow · arma las imágenes del instalador NSIS a partir del ícono de la app.

    python scripts/instalador-imagenes.py

NSIS no acepta PNG ni transparencia: pide BMP de 24 bits con fondo blanco (el asistente es blanco).
Y pide medidas exactas: el encabezado son 150x57 y la barra lateral 164x314. Si te equivocás, el
instalador sale sin marca (o directamente falla el empaquetado).

Salida (se versionan en el repo, son la cara del instalador):
    src-tauri/installer/encabezado.bmp      150x57   → páginas internas + desinstalador
    src-tauri/installer/lateral.bmp         164x314  → páginas de bienvenida y de cierre

Los colores salen del propio ícono (turquesa #24C8DB, ámbar #FFC131), no se eligen a mano.
"""
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

RAIZ = Path(__file__).resolve().parent.parent
ICONO = RAIZ / "src-tauri" / "icons" / "icon.png"
SALIDA = RAIZ / "src-tauri" / "installer"

BLANCO = (255, 255, 255)
TINTA = (33, 49, 60)        # gris azulado oscuro para el nombre
GRIS = (122, 138, 148)      # para la bajada
TURQUESA = (36, 200, 219)   # #24C8DB
FUENTE_BOLD = Path("C:/Windows/Fonts/segoeuib.ttf")
FUENTE_REG = Path("C:/Windows/Fonts/segoeui.ttf")


def marca(lado: int) -> Image.Image:
    """El ícono recortado a su arte (sin el margen transparente) y cuadrado."""
    im = Image.open(ICONO).convert("RGBA")
    im = im.crop(im.split()[3].getbbox())
    escala = lado / max(im.size)
    im = im.resize((max(1, round(im.width * escala)), max(1, round(im.height * escala))), Image.LANCZOS)
    caja = Image.new("RGBA", (lado, lado), (0, 0, 0, 0))
    caja.paste(im, ((lado - im.width) // 2, (lado - im.height) // 2), im)
    return caja


def pegar(base: Image.Image, capa: Image.Image, x: int, y: int) -> None:
    base.paste(capa, (x, y), capa)


def centrado(dib: ImageDraw.ImageDraw, texto: str, fuente, ancho: int, y: int, color) -> None:
    caja = dib.textbbox((0, 0), texto, font=fuente)
    dib.text(((ancho - (caja[2] - caja[0])) / 2 - caja[0], y), texto, font=fuente, fill=color)


def encabezado() -> Image.Image:
    """150x57 · el ícono a la izquierda y el nombre al lado (lockup)."""
    im = Image.new("RGB", (150, 57), BLANCO)
    dib = ImageDraw.Draw(im)
    pegar(im, marca(40), 8, 8)
    fuente = ImageFont.truetype(str(FUENTE_BOLD), 19)
    caja = dib.textbbox((0, 0), "NodeFlow", font=fuente)
    dib.text((56, (57 - (caja[3] - caja[1])) / 2 - caja[1]), "NodeFlow", font=fuente, fill=TINTA)
    return im


def lateral() -> Image.Image:
    """164x314 · el ícono grande arriba, el nombre y la bajada, todo centrado."""
    im = Image.new("RGB", (164, 314), BLANCO)
    dib = ImageDraw.Draw(im)
    pegar(im, marca(96), 34, 46)

    nombre = ImageFont.truetype(str(FUENTE_BOLD), 21)
    centrado(dib, "NodeFlow", nombre, 164, 158, TINTA)
    dib.rectangle([62, 188, 102, 190], fill=TURQUESA)  # regla fina

    bajada = ImageFont.truetype(str(FUENTE_REG), 10)
    centrado(dib, "lienzo local-first de", bajada, 164, 200, GRIS)
    centrado(dib, "pensamiento aumentado", bajada, 164, 214, GRIS)
    return im


def main() -> None:
    SALIDA.mkdir(parents=True, exist_ok=True)
    for nombre, img, medida in (
        ("encabezado.bmp", encabezado(), (150, 57)),
        ("lateral.bmp", lateral(), (164, 314)),
    ):
        assert img.size == medida, f"{nombre}: {img.size} != {medida}"
        destino = SALIDA / nombre
        img.convert("RGB").save(destino, "BMP")
        print(f"  {destino.relative_to(RAIZ)}  {img.size[0]}x{img.size[1]}  {destino.stat().st_size} B")


if __name__ == "__main__":
    main()
