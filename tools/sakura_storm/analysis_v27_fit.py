import os
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
FONT_PATH = os.getenv("SS_FONT_PATH", "/System/Library/Fonts/Hiragino Sans GB.ttc")
FONT_INDEX = int(os.getenv("SS_FONT_INDEX", "2"))
GRID_N_HINT = int(os.getenv("SS_GRID_N_HINT", "32"))
CELL_HINT = int(os.getenv("SS_CELL_SIZE_HINT", "16"))
OFFSET_HINT = int(os.getenv("SS_GRID_OFFSET_HINT", "0"))
DATA_R_HINT = float(os.getenv("SS_DATA_R_HINT", "247.0"))
RADIAL_THRESH = float(os.getenv("SS_RADIAL_THRESH", "0.1"))


def load_frames(path: str) -> np.ndarray:
    im = Image.open(path)
    frames: List[np.ndarray] = []
    for i in range(im.n_frames):
        im.seek(i)
        frames.append(np.array(im.convert("RGB"), dtype=np.uint8))
    return np.stack(frames, axis=0)


def palette_from_frames(frames: np.ndarray) -> List[Tuple[int, int, int]]:
    colors: Dict[Tuple[int, int, int], int] = {}
    for f in frames:
        uniq = np.unique(f.reshape(-1, 3), axis=0)
        for c in uniq:
            colors.setdefault((int(c[0]), int(c[1]), int(c[2])), 0)
    return sorted(colors.keys())


def classify_palette(palette: List[Tuple[int, int, int]]) -> Tuple[List[Tuple[int, int, int]], List[Tuple[int, int, int]]]:
    bright = []
    dim = []
    for r, g, b in palette:
        if max(r, g, b) >= 200:
            bright.append((r, g, b))
        elif 55 <= r <= 85 and 28 <= g <= 55 and 35 <= b <= 70:
            dim.append((r, g, b))
    return bright, dim


def build_mask_any(frames: np.ndarray, bright: List[Tuple[int, int, int]], dim: List[Tuple[int, int, int]]) -> np.ndarray:
    if not bright or not dim:
        raise RuntimeError("Could not classify bright/dim colors from palette.")
    mask = np.zeros(frames.shape[:3], dtype=bool)
    for col in bright + dim:
        mask |= np.all(frames == np.array(col, dtype=np.uint8), axis=3)
    return np.any(mask, axis=0)


def radial_density(mask_any: np.ndarray, max_r: int = 260) -> Tuple[np.ndarray, np.ndarray]:
    h, w = mask_any.shape
    cy = h / 2.0
    cx = w / 2.0
    yy, xx = np.indices((h, w))
    rr = np.sqrt((xx - cx) ** 2 + (yy - cy) ** 2)
    bins = np.linspace(0, float(max_r), max_r + 1)
    counts = np.zeros(len(bins) - 1, dtype=np.float64)
    area = np.zeros(len(bins) - 1, dtype=np.float64)
    for i in range(len(bins) - 1):
        lo, hi = bins[i], bins[i + 1]
        in_bin = (rr >= lo) & (rr < hi)
        area[i] = float(in_bin.sum())
        counts[i] = float(mask_any[in_bin].sum())
    dens = np.zeros_like(counts)
    nz = area > 0
    dens[nz] = counts[nz] / area[nz]
    return dens, bins


def low_density_ranges(dens: np.ndarray, bins: np.ndarray, thresh: float) -> List[Tuple[int, int]]:
    ranges: List[Tuple[int, int]] = []
    start = None
    for i, val in enumerate(dens):
        if val < thresh and start is None:
            start = i
        if val >= thresh and start is not None:
            ranges.append((start, i - 1))
            start = None
    if start is not None:
        ranges.append((start, len(dens) - 1))
    out: List[Tuple[int, int]] = []
    for s, e in ranges:
        out.append((int(round(bins[s])), int(round(bins[e + 1]))))
    return out


def cell_activity(mask_frames: np.ndarray, grid_n: int, cell: int, offset: int, data_r: float) -> np.ndarray:
    f, h, w = mask_frames.shape
    cx = w / 2.0
    cy = h / 2.0
    if offset == 0 and h == grid_n * cell and w == grid_n * cell:
        mask_cells = mask_frames.reshape(f, grid_n, cell, grid_n, cell)
        cell_any = mask_cells.any(axis=(2, 4))
    else:
        cell_any = np.zeros((f, grid_n, grid_n), dtype=bool)
        for r in range(grid_n):
            y0 = offset + r * cell
            for c in range(grid_n):
                x0 = offset + c * cell
                if x0 < 0 or y0 < 0 or x0 + cell > w or y0 + cell > h:
                    continue
                region = mask_frames[:, y0 : y0 + cell, x0 : x0 + cell]
                cell_any[:, r, c] = np.any(region, axis=(1, 2))
    coords = np.indices((grid_n, grid_n))
    ys = coords[0] * cell + cell / 2.0 + offset
    xs = coords[1] * cell + cell / 2.0 + offset
    inside = (xs - cx) ** 2 + (ys - cy) ** 2 <= data_r ** 2
    cell_active = cell_any.mean(axis=0)
    cell_active[~inside] = 0.0
    return cell_active


def eval_grid(mask_any: np.ndarray, cell: int, grid_n: int, offset: int) -> Tuple[float, float, int]:
    h, w = mask_any.shape
    cx = w / 2.0
    cy = h / 2.0
    active = []
    rs = []
    for r in range(grid_n):
        y0 = offset + r * cell
        for c in range(grid_n):
            x0 = offset + c * cell
            if x0 < 0 or y0 < 0 or x0 + cell > w or y0 + cell > h:
                continue
            region = mask_any[y0 : y0 + cell, x0 : x0 + cell]
            is_active = bool(region.any())
            active.append(is_active)
            cx_cell = x0 + cell / 2.0
            cy_cell = y0 + cell / 2.0
            rs.append(((cx_cell - cx) ** 2 + (cy_cell - cy) ** 2) ** 0.5)
    active = np.array(active, dtype=bool)
    rs = np.array(rs, dtype=np.float32)
    radii = np.linspace(150.0, 260.0, 23)
    best_err = 1e9
    best_r = radii[0]
    for r0 in radii:
        pred = rs <= r0
        err = float(np.mean(pred != active))
        if err < best_err:
            best_err = err
            best_r = r0
    return best_err, float(best_r), len(active)


def glyph_stats(cell: int, thresh: int = 64) -> Tuple[float, float]:
    font = ImageFont.truetype(FONT_PATH, max(8, cell), index=FONT_INDEX)
    katakana = list("アイウエオカキクケコサシスセソタチツテトナニヌネノハヒフヘホマミムメモヤユヨラリルレロワヲン")
    counts = []
    for g in katakana:
        img = Image.new("L", (cell, cell), 0)
        draw = ImageDraw.Draw(img)
        bbox = draw.textbbox((0, 0), g, font=font)
        gw = bbox[2] - bbox[0]
        gh = bbox[3] - bbox[1]
        tx = cell / 2 - gw / 2
        ty = cell / 2 - gh / 2
        draw.text((tx, ty), g, font=font, fill=255)
        arr = np.array(img)
        counts.append(int((arr > thresh).sum()))
    return float(np.mean(counts)), float(np.std(counts))


def main() -> None:
    frames = load_frames(REF_GIF)
    palette = palette_from_frames(frames)
    bright, dim = classify_palette(palette)
    mask_any = build_mask_any(frames, bright, dim)
    mask_frames = np.zeros(frames.shape[:3], dtype=bool)
    for col in bright + dim:
        mask_frames |= np.all(frames == np.array(col, dtype=np.uint8), axis=3)

    h, w = mask_any.shape
    candidates = []
    for cell in range(12, 19):
        for grid_n in range(24, 37):
            span = grid_n * cell
            if span > w or span > h:
                continue
            base_off = int(round((w - span) / 2.0))
            for delta in range(-2, 3):
                offset = base_off + delta
                err, r0, count = eval_grid(mask_any, cell, grid_n, offset)
                candidates.append((err, cell, grid_n, offset, r0, count))

    candidates.sort(key=lambda x: x[0])
    print("Top grid candidates (err, cell, grid_n, offset, radius, cells):")
    for c in candidates[:10]:
        print(c)

    print("\nGlyph area stats (mean, std) for candidate cell sizes:")
    for cell in range(12, 19):
        mean, std = glyph_stats(cell, thresh=64)
        print(cell, f"{mean:.1f}", f"{std:.1f}")

    dens, bins = radial_density(mask_any)
    ranges = low_density_ranges(dens, bins, RADIAL_THRESH)
    print("\nLow-density radial ranges (density <", RADIAL_THRESH, "):")
    for lo, hi in ranges:
        print(f"{lo}-{hi}")

    cell_active = cell_activity(mask_frames, GRID_N_HINT, CELL_HINT, OFFSET_HINT, DATA_R_HINT)
    active = cell_active[cell_active > 0]
    if active.size:
        print("\nCell activity stats (hinted grid):")
        print("cells", int(active.size))
        print("active mean", float(active.mean()), "std", float(active.std()))
        print("min", float(active.min()), "max", float(active.max()))
        print("inactive (<0.1)", int(np.sum(cell_active > 0) - np.sum(cell_active >= 0.1)))
        print("always on (>=0.9)", int(np.sum(cell_active >= 0.9)))


if __name__ == "__main__":
    main()
