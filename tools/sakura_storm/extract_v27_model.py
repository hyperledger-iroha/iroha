import os
from typing import Dict, List, Tuple

import numpy as np
from PIL import Image


BASE_DIR = os.path.dirname(__file__)
DEFAULT_REF = os.path.join(
    BASE_DIR,
    "sakura_storm_encoded_preview_logo_3rings_v27_fullwidth_katakana_lowdensity_bright.gif",
)
REF_GIF = os.getenv(
    "SS_REF_GIF",
    DEFAULT_REF
    if os.path.exists(DEFAULT_REF)
    else "/tmp/sakura_storm_viz/sakura_storm_encoded_preview_logo_3rings_v27_fullwidth_katakana_lowdensity_bright.gif",
)
OUT_NPZ = os.getenv("SS_OUT_NPZ", os.path.join(BASE_DIR, "v27_model.npz"))


def main() -> None:
    im = Image.open(REF_GIF)
    frames: List[np.ndarray] = []
    for i in range(im.n_frames):
        im.seek(i)
        frames.append(np.array(im.convert("RGB"), dtype=np.uint8))
    frames_np = np.stack(frames, axis=0)

    # Build palette across all frames
    colors: Dict[Tuple[int, int, int], int] = {}
    for f in frames_np:
        uniq = np.unique(f.reshape(-1, 3), axis=0)
        for c in uniq:
            colors.setdefault((int(c[0]), int(c[1]), int(c[2])), 0)
    palette = sorted(colors.keys())
    palette_arr = np.array(palette, dtype=np.uint8)
    color_to_idx = {c: i for i, c in enumerate(palette)}

    # Map frames to palette indices
    idx_frames = np.zeros(frames_np.shape[:3], dtype=np.uint8)
    for t in range(frames_np.shape[0]):
        f = frames_np[t]
        flat = f.reshape(-1, 3)
        idx = np.zeros(flat.shape[0], dtype=np.uint8)
        for i, pix in enumerate(flat):
            key = (int(pix[0]), int(pix[1]), int(pix[2]))
            if key not in color_to_idx:
                raise RuntimeError(f"Unexpected color {key} in frame {t}")
            idx[i] = color_to_idx[key]
        idx_frames[t] = idx.reshape(frames_np.shape[1:3])

    duration = int(im.info.get("duration", 40))
    loop = int(im.info.get("loop", 0))
    np.savez_compressed(
        OUT_NPZ,
        palette=palette_arr,
        frames_idx=idx_frames,
        duration=duration,
        loop=loop,
        size=np.array([frames_np.shape[2], frames_np.shape[1]], dtype=np.int32),
    )
    print("model", OUT_NPZ)
    print("palette", palette)


if __name__ == "__main__":
    main()
