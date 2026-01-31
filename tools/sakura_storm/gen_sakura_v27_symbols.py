import math
import os
from typing import List, Tuple

import numpy as np
from PIL import Image, ImageDraw, ImageFont

BASE_DIR = os.path.dirname(__file__)
SYMBOLS_NPZ = os.getenv(
    "SS_SYMBOLS_NPZ",
    "/tmp/sakura_storm_viz/v27_symbols.npz"
    if os.path.isdir("/tmp/sakura_storm_viz")
    else os.path.join(BASE_DIR, "v27_symbols.npz"),
)
OUT_GIF = os.getenv(
    "SS_OUT_GIF",
    "/tmp/sakura_storm_viz/sakura_storm_v27_from_symbols.gif"
    if os.path.isdir("/tmp/sakura_storm_viz")
    else os.path.join(BASE_DIR, "sakura_storm_v27_from_symbols.gif"),
)

W = int(os.getenv("SS_W", "512"))
H = int(os.getenv("SS_H", "512"))
FPS = int(os.getenv("SS_FPS", "25"))

# Rings
RING_RADII = [float(x) for x in os.getenv("SS_RING_RADII", "190,214,238").split(",") if x.strip()]
RING_SPACING = float(os.getenv("SS_RING_SPACING", "37.0"))
RING_DOT_R = int(os.getenv("SS_RING_DOT_R", "7"))
RING_DIM_ANGLES = [0.0, 90.0, 180.0, 270.0]

# Logo
LOGO_PATH = os.getenv(
    "SS_LOGO_PATH",
    "/Users/mtakemiya/Library/CloudStorage/Dropbox/soramitsu/tarmo/sora_logo.png",
)
LOGO_SCALE = float(os.getenv("SS_LOGO_SCALE", "0.318"))
LOGO_ALPHA = int(os.getenv("SS_LOGO_ALPHA", "255"))

# Glyphs
FONT_PATH = os.getenv("SS_FONT_PATH", "/System/Library/Fonts/Hiragino Sans GB.ttc")
FONT_INDEX = int(os.getenv("SS_FONT_INDEX", "0"))
GLYPH_SIZE = int(os.getenv("SS_GLYPH_SIZE", "15"))
GLYPH_THRESH = int(os.getenv("SS_GLYPH_THRESH", "96"))


def render_glyph_mask(font: ImageFont.FreeTypeFont, glyph: str, cell: int) -> np.ndarray:
    img = Image.new("L", (cell, cell), 0)
    draw = ImageDraw.Draw(img)
    bbox = draw.textbbox((0, 0), glyph, font=font)
    gw = bbox[2] - bbox[0]
    gh = bbox[3] - bbox[1]
    tx = cell / 2 - gw / 2
    ty = cell / 2 - gh / 2
    draw.text((tx, ty), glyph, font=font, fill=255)
    arr = np.array(img)
    return arr > GLYPH_THRESH


def build_logo_layer(w: int, h: int, logo_shades: np.ndarray) -> Image.Image:
    if not os.path.exists(LOGO_PATH):
        return Image.new("RGBA", (w, h), (0, 0, 0, 0))
    logo = Image.open(LOGO_PATH).convert("RGBA")
    target = int(min(w, h) * LOGO_SCALE)
    target = max(8, min(min(w, h), target))
    logo = logo.resize((target, target), resample=Image.LANCZOS)
    arr = np.array(logo, dtype=np.uint8)
    alpha = arr[..., 3].astype(np.float32) / 255.0
    tinted = np.zeros_like(arr)
    tint = tuple(int(x) for x in logo_shades[-1])
    tinted[..., 0] = tint[0]
    tinted[..., 1] = tint[1]
    tinted[..., 2] = tint[2]
    tinted[..., 3] = np.clip(alpha * LOGO_ALPHA, 0, 255).astype(np.uint8)
    logo_tinted = Image.fromarray(tinted, mode="RGBA")
    full = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    x0 = int(w / 2 - target / 2)
    y0 = int(h / 2 - target / 2)
    full.paste(logo_tinted, (x0, y0), logo_tinted)
    return full


def build_ring_layer(w: int, h: int, ring_bright: np.ndarray, ring_dim: np.ndarray) -> Image.Image:
    layer = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    draw = ImageDraw.Draw(layer)
    for r in RING_RADII:
        n = max(8, int(round((2.0 * math.pi * r) / max(1.0, RING_SPACING))))
        for k in range(n):
            ang = 2.0 * math.pi * k / n
            x = w / 2 + r * math.cos(ang)
            y = h / 2 + r * math.sin(ang)
            color = ring_bright
            if abs(r - 214.0) < 0.8:
                deg = (math.degrees(ang) + 360.0) % 360.0
                for a in RING_DIM_ANGLES:
                    if abs((deg - a + 180.0) % 360.0 - 180.0) < 4.0:
                        color = ring_dim
                        break
            draw.ellipse(
                (x - RING_DOT_R, y - RING_DOT_R, x + RING_DOT_R, y + RING_DOT_R),
                fill=tuple(int(c) for c in color),
            )
    return layer


def main() -> None:
    data = np.load(SYMBOLS_NPZ, allow_pickle=False)
    cells = data["cells"]
    glyph_idx = data["glyph_idx"]
    bits = data["bits"]
    glyphs = [str(g) for g in data["glyphs"].tolist()]
    mask_bits = data.get("mask_bits")
    glyph_masks_ref = data.get("glyph_masks_ref")
    glyph_masks_packed = data.get("glyph_masks")
    allowed_masks = data.get("allowed_masks")
    ring_idx = data.get("ring_idx")
    ring_layer = data.get("ring_layer")
    cell_size = int(data.get("cell_size", 16))
    bg = data.get("bg", np.array([11, 7, 12], dtype=np.uint8))
    data_bright = data.get("data_bright", np.array([255, 233, 246], dtype=np.uint8))
    data_dim = data.get("data_dim", np.array([69, 40, 54], dtype=np.uint8))
    ring_bright = data.get("ring_bright", np.array([255, 246, 252], dtype=np.uint8))
    ring_dim = data.get("ring_dim", np.array([62, 36, 50], dtype=np.uint8))
    logo_shades = data.get("logo_shades", np.array([[30, 16, 25], [36, 18, 28], [43, 20, 32]], dtype=np.uint8))
    static_layer = data.get("static_layer")

    steps = glyph_idx.shape[0]
    if bits.shape != glyph_idx.shape:
        raise ValueError("bits shape does not match glyph_idx")

    glyph_masks = None
    if glyph_masks_packed is not None:
        glyph_masks = []
        for row in glyph_masks_packed:
            flat = np.unpackbits(row, bitorder="big")[: cell_size * cell_size]
            glyph_masks.append(flat.reshape((cell_size, cell_size)).astype(bool))
    elif glyph_masks_ref is not None:
        glyph_masks = []
        for row in glyph_masks_ref:
            flat = np.unpackbits(row, bitorder="big")[: cell_size * cell_size]
            glyph_masks.append(flat.reshape((cell_size, cell_size)).astype(bool))
    elif mask_bits is None:
        font = ImageFont.truetype(FONT_PATH, GLYPH_SIZE, index=FONT_INDEX)
        glyph_masks = [render_glyph_mask(font, g, cell_size) for g in glyphs]

    logo_layer = None if static_layer is not None else build_logo_layer(W, H, logo_shades)
    ring_layer_img = None
    if ring_layer is None and static_layer is None and ring_idx is None:
        ring_layer_img = build_ring_layer(W, H, ring_bright, ring_dim)

    frames: List[Image.Image] = []
    for f in range(steps):
        if static_layer is not None:
            base = np.array(static_layer, dtype=np.uint8)
        else:
            base = np.zeros((H, W, 3), dtype=np.uint8)
            base[:] = bg
        if ring_layer is not None:
            mask = (ring_layer != np.array([0, 0, 0], dtype=np.uint8)).any(axis=2)
            if mask.any():
                base[mask] = ring_layer[mask]
        frame = Image.fromarray(base, mode="RGB").convert("RGBA")
        draw_arr = np.array(frame, dtype=np.uint8)
        for i, (x0, y0) in enumerate(cells):
            if mask_bits is not None:
                packed = mask_bits[f, i]
                flat = np.unpackbits(packed, bitorder="big")[: cell_size * cell_size]
                mask = flat.reshape((cell_size, cell_size)).astype(bool)
            else:
                mask = glyph_masks[int(glyph_idx[f, i])]
                if allowed_masks is not None:
                    allowed_flat = np.unpackbits(allowed_masks[i], bitorder="big")[: cell_size * cell_size]
                    allowed = allowed_flat.reshape((cell_size, cell_size)).astype(bool)
                    mask = mask & allowed
            color = data_bright if int(bits[f, i]) == 1 else data_dim
            region = draw_arr[y0 : y0 + cell_size, x0 : x0 + cell_size]
            region[mask] = (int(color[0]), int(color[1]), int(color[2]), 255)
        frame = Image.fromarray(draw_arr, mode="RGBA")
        if ring_idx is not None:
            arr = np.array(frame, dtype=np.uint8)
            mask_b = ring_idx[f] == 1
            mask_d = ring_idx[f] == 2
            if mask_b.any():
                arr[mask_b] = (int(ring_bright[0]), int(ring_bright[1]), int(ring_bright[2]), 255)
            if mask_d.any():
                arr[mask_d] = (int(ring_dim[0]), int(ring_dim[1]), int(ring_dim[2]), 255)
            frame = Image.fromarray(arr, mode="RGBA")
        elif ring_layer_img is not None:
            frame = Image.alpha_composite(frame, ring_layer_img)
        if logo_layer is not None:
            frame = Image.alpha_composite(frame, logo_layer)
        frames.append(frame.convert("RGB"))

    frames[0].save(
        OUT_GIF,
        save_all=True,
        append_images=frames[1:],
        duration=int(1000 / FPS),
        loop=0,
        disposal=2,
    )
    print("gif", OUT_GIF)


if __name__ == "__main__":
    main()
