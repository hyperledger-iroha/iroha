import math
import os
from dataclasses import dataclass
from typing import Dict, List, Tuple

import numpy as np
from PIL import Image, ImageDraw, ImageFont

BASE_DIR = os.path.dirname(__file__)
REF_GIF = os.getenv(
    "SS_REF_GIF",
    os.path.join(
        BASE_DIR,
        "sakura_storm_encoded_preview_logo_3rings_v27_fullwidth_katakana_lowdensity_bright.gif",
    ),
)
OUT_NPZ = os.getenv(
    "SS_OUT_NPZ",
    "/tmp/sakura_storm_viz/v27_symbols.npz"
    if os.path.isdir("/tmp/sakura_storm_viz")
    else os.path.join(BASE_DIR, "v27_symbols.npz"),
)

W = int(os.getenv("SS_W", "512"))
H = int(os.getenv("SS_H", "512"))
GRID_N = int(os.getenv("SS_GRID_N", "32"))
CELL_SIZE = int(os.getenv("SS_CELL_SIZE", "16"))
DATA_RADIUS = float(os.getenv("SS_DATA_RADIUS", "247.0"))

# Palette (v27)
BG = (11, 7, 12)
DATA_BRIGHT = (255, 233, 246)
DATA_DIM = (69, 40, 54)
RING_BRIGHT = (255, 246, 252)
RING_DIM = (62, 36, 50)
LOGO_SHADES = [(30, 16, 25), (36, 18, 28), (43, 20, 32)]

# Glyphs
KATAKANA = list("アイウエオカキクケコサシスセソタチツテトナニヌネノハヒフヘホマミムメモヤユヨラリルレロワヲン")
FONT_PATH = os.getenv("SS_FONT_PATH", "/System/Library/Fonts/Hiragino Sans GB.ttc")
FONT_INDEX = int(os.getenv("SS_FONT_INDEX", "0"))
GLYPH_SIZE = int(os.getenv("SS_GLYPH_SIZE", "15"))
GLYPH_THRESH = int(os.getenv("SS_GLYPH_THRESH", "96"))


@dataclass
class Cell:
    x0: int
    y0: int
    rr: float


def load_frames(path: str) -> np.ndarray:
    im = Image.open(path)
    frames = []
    for i in range(im.n_frames):
        im.seek(i)
        frames.append(np.array(im.convert("RGB"), dtype=np.uint8))
    return np.stack(frames, axis=0)


def render_glyph_mask(font: ImageFont.FreeTypeFont, glyph: str) -> np.ndarray:
    img = Image.new("L", (CELL_SIZE, CELL_SIZE), 0)
    draw = ImageDraw.Draw(img)
    bbox = draw.textbbox((0, 0), glyph, font=font)
    gw = bbox[2] - bbox[0]
    gh = bbox[3] - bbox[1]
    tx = CELL_SIZE / 2 - gw / 2
    ty = CELL_SIZE / 2 - gh / 2
    draw.text((tx, ty), glyph, font=font, fill=255)
    arr = np.array(img)
    return arr > GLYPH_THRESH


def build_low_pool(font: ImageFont.FreeTypeFont) -> List[str]:
    densities: List[Tuple[str, float]] = []
    for g in KATAKANA:
        mask = render_glyph_mask(font, g)
        densities.append((g, float(np.mean(mask))))
    densities.sort(key=lambda x: x[1])
    return [g for g, _ in densities[: max(4, len(densities) // 3)]]


def build_cells(frames: np.ndarray) -> List[Cell]:
    cx = W / 2.0
    cy = H / 2.0
    cells: List[Cell] = []
    for r in range(GRID_N):
        y0 = r * CELL_SIZE
        for c in range(GRID_N):
            x0 = c * CELL_SIZE
            cell_cx = x0 + CELL_SIZE / 2.0
            cell_cy = y0 + CELL_SIZE / 2.0
            rr = math.hypot(cell_cx - cx, cell_cy - cy)
            if rr > DATA_RADIUS:
                continue
            # keep only cells that show any data color across frames
            tile = frames[:, y0 : y0 + CELL_SIZE, x0 : x0 + CELL_SIZE]
            data_any = (tile == np.array(DATA_BRIGHT, dtype=np.uint8)).all(axis=3) | (
                tile == np.array(DATA_DIM, dtype=np.uint8)
            ).all(axis=3)
            if not data_any.any():
                continue
            cells.append(Cell(x0=x0, y0=y0, rr=rr))
    return cells


def main() -> None:
    frames = load_frames(REF_GIF)
    if frames.shape[1] != H or frames.shape[2] != W:
        raise ValueError("reference GIF size does not match SS_W/SS_H")

    palette: List[Tuple[int, int, int]] = [
        BG,
        LOGO_SHADES[0],
        LOGO_SHADES[1],
        LOGO_SHADES[2],
        RING_DIM,
        DATA_DIM,
        DATA_BRIGHT,
        RING_BRIGHT,
    ]
    pal_arr = np.array(palette, dtype=np.uint8)

    font = ImageFont.truetype(FONT_PATH, GLYPH_SIZE, index=FONT_INDEX)
    glyphs = build_low_pool(font)
    glyph_masks = [render_glyph_mask(font, g) for g in glyphs]
    glyph_lookup: Dict[bytes, int] = {}
    for idx, mask in enumerate(glyph_masks):
        key = np.packbits(mask.reshape(-1).astype(np.uint8)).tobytes()
        glyph_lookup[key] = idx

    cells = build_cells(frames)
    if not cells:
        raise RuntimeError("no data cells detected")

    steps = frames.shape[0]
    glyph_idx = np.zeros((steps, len(cells)), dtype=np.uint8)
    bits = np.zeros((steps, len(cells)), dtype=np.uint8)
    packed_len = (CELL_SIZE * CELL_SIZE + 7) // 8

    bright = np.array(DATA_BRIGHT, dtype=np.uint8)
    dim = np.array(DATA_DIM, dtype=np.uint8)

    ring_b = (frames == np.array(RING_BRIGHT, dtype=np.uint8)).all(axis=3)
    ring_d = (frames == np.array(RING_DIM, dtype=np.uint8)).all(axis=3)
    ring_mask = ring_b | ring_d
    ring_any = ring_mask.any(axis=0)
    b_count = ring_b.sum(axis=0)
    d_count = ring_d.sum(axis=0)
    ring_layer = np.zeros((H, W, 3), dtype=np.uint8)
    ring_layer[ring_any] = np.array(RING_BRIGHT, dtype=np.uint8)
    ring_layer[d_count > b_count] = np.array(RING_DIM, dtype=np.uint8)
    logo_mask = np.zeros(frames.shape[:3], dtype=bool)
    for col in LOGO_SHADES:
        logo_mask |= (frames == np.array(col, dtype=np.uint8)).all(axis=3)
    logo_any = logo_mask.any(axis=0)

    # Build static layer: per-pixel mode, excluding any pixel that ever carries data colors.
    data_mask = (frames == bright).all(axis=3) | (frames == dim).all(axis=3)
    data_any = data_mask.any(axis=0)
    counts = np.zeros((len(palette), H, W), dtype=np.uint16)
    for f in range(steps):
        frame = frames[f]
        idx = np.zeros((H, W), dtype=np.uint8)
        for i, col in enumerate(pal_arr):
            idx[(frame == col).all(axis=2)] = i
        for i in range(len(palette)):
            counts[i] += (idx == i)
    static_idx = counts.argmax(axis=0).astype(np.uint8)
    bg_idx = palette.index(BG)
    logo_indices = [palette.index(LOGO_SHADES[0]), palette.index(LOGO_SHADES[1]), palette.index(LOGO_SHADES[2])]
    logo_counts = counts[logo_indices]
    logo_choice = np.array(logo_indices, dtype=np.uint8)[logo_counts.argmax(axis=0)]
    static_idx[logo_any] = logo_choice[logo_any]
    static_idx[data_any & (~logo_any)] = bg_idx
    static_layer = pal_arr[static_idx]

    for f in range(steps):
        frame = frames[f]
        for i, cell in enumerate(cells):
            x0 = cell.x0
            y0 = cell.y0
            tile = frame[y0 : y0 + CELL_SIZE, x0 : x0 + CELL_SIZE]
            bmask = (tile == bright).all(axis=2)
            dmask = (tile == dim).all(axis=2)
            if bmask.any() and dmask.any():
                # unexpected mixed colors; fall back to majority
                bits[f, i] = 1 if bmask.sum() >= dmask.sum() else 0
            elif bmask.any():
                bits[f, i] = 1
            elif dmask.any():
                bits[f, i] = 0
            else:
                raise RuntimeError("missing data colors in tile")
            union = bmask | dmask
            ring_tile = ring_mask[:, y0 : y0 + CELL_SIZE, x0 : x0 + CELL_SIZE].any(axis=0)
            logo_tile = logo_mask[:, y0 : y0 + CELL_SIZE, x0 : x0 + CELL_SIZE].any(axis=0)
            idx = None
            if not ring_tile.any() and not logo_tile.any():
                key = np.packbits(union.reshape(-1).astype(np.uint8)).tobytes()
                idx = glyph_lookup.get(key)
            if idx is None:
                # fall back to best match (ignore ring pixels if present)
                best_idx = 0
                best = 1.0
                if ring_tile.any() or logo_tile.any():
                    valid = ~(ring_tile | logo_tile)
                    if not valid.any():
                        valid = np.ones_like(ring_tile, dtype=bool)
                else:
                    valid = np.ones_like(union, dtype=bool)
                for j, gm in enumerate(glyph_masks):
                    diff = float(np.mean(union[valid] != gm[valid]))
                    if diff < best:
                        best = diff
                        best_idx = j
                idx = best_idx
            glyph_idx[f, i] = idx

    # Precompute allowed masks per cell (data-any) to model logo clipping.
    allowed_masks = np.zeros((len(cells), packed_len), dtype=np.uint8)
    for i, cell in enumerate(cells):
        allowed = data_any[cell.y0 : cell.y0 + CELL_SIZE, cell.x0 : cell.x0 + CELL_SIZE]
        allowed_masks[i] = np.packbits(allowed.reshape(-1).astype(np.uint8))

    # Build low-density glyph masks and map each frame/cell to a glyph index.
    glyph_masks = [render_glyph_mask(font, g) for g in glyphs]
    glyph_masks_packed = np.stack(
        [np.packbits(m.reshape(-1).astype(np.uint8)) for m in glyph_masks],
        axis=0,
    )

    glyph_idx_ref = np.zeros((steps, len(cells)), dtype=np.uint8)
    for f in range(steps):
        for i, cell in enumerate(cells):
            union = (frames[f, cell.y0 : cell.y0 + CELL_SIZE, cell.x0 : cell.x0 + CELL_SIZE] == bright).all(axis=2) | (
                frames[f, cell.y0 : cell.y0 + CELL_SIZE, cell.x0 : cell.x0 + CELL_SIZE] == dim
            ).all(axis=2)
            if not union.any():
                raise RuntimeError("missing data colors in tile")
            allowed = np.unpackbits(allowed_masks[i], bitorder="big")[: CELL_SIZE * CELL_SIZE].reshape(
                (CELL_SIZE, CELL_SIZE)
            )
            best_idx = 0
            best = 1.0
            for gi, gm in enumerate(glyph_masks):
                diff = float(np.mean(union != (gm & allowed)))
                if diff < best:
                    best = diff
                    best_idx = gi
                    if best == 0.0:
                        break
            if best > 0.0:
                raise RuntimeError("no exact glyph match for tile")
            glyph_idx_ref[f, i] = best_idx

    np.savez(
        OUT_NPZ,
        cells=np.array([[c.x0, c.y0] for c in cells], dtype=np.int16),
        glyph_idx=glyph_idx_ref,
        bits=bits,
        glyph_masks=glyph_masks_packed,
        allowed_masks=allowed_masks,
        glyphs=np.array(glyphs),
        static_layer=static_layer.astype(np.uint8),
        palette=pal_arr,
        ring_layer=ring_layer,
        cell_size=np.array(CELL_SIZE, dtype=np.int32),
        grid_n=np.array(GRID_N, dtype=np.int32),
        data_radius=np.array(DATA_RADIUS, dtype=np.float32),
        font_index=np.array(FONT_INDEX, dtype=np.int32),
        glyph_size=np.array(GLYPH_SIZE, dtype=np.int32),
        glyph_thresh=np.array(GLYPH_THRESH, dtype=np.int32),
        bg=np.array(BG, dtype=np.uint8),
        data_bright=np.array(DATA_BRIGHT, dtype=np.uint8),
        data_dim=np.array(DATA_DIM, dtype=np.uint8),
        ring_bright=np.array(RING_BRIGHT, dtype=np.uint8),
        ring_dim=np.array(RING_DIM, dtype=np.uint8),
        logo_shades=np.array(LOGO_SHADES, dtype=np.uint8),
    )
    print("symbols", OUT_NPZ)


if __name__ == "__main__":
    main()
