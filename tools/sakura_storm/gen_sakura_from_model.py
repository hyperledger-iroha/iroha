import os
from typing import Tuple

import numpy as np
from PIL import Image


BASE_DIR = os.path.dirname(__file__)
MODEL_NPZ = os.getenv("SS_MODEL_NPZ", os.path.join(BASE_DIR, "v27_model.npz"))
DEFAULT_OUT = "/tmp/sakura_storm_viz/sakura_storm_v27_from_model.gif"
OUT_GIF = os.getenv(
    "SS_OUT_GIF",
    DEFAULT_OUT if os.path.isdir("/tmp/sakura_storm_viz") else os.path.join(BASE_DIR, "sakura_storm_v27_from_model.gif"),
)


def parse_color(text: str, default: Tuple[int, int, int]) -> Tuple[int, int, int]:
    parts = [p.strip() for p in text.split(",") if p.strip()]
    if len(parts) != 3:
        return default
    try:
        vals = tuple(int(max(0, min(255, int(p)))) for p in parts)
    except ValueError:
        return default
    return vals


def main() -> None:
    data = np.load(MODEL_NPZ)
    palette = data["palette"].astype(np.uint8)
    frames_idx = data["frames_idx"].astype(np.uint8)
    duration = int(data["duration"]) if "duration" in data else 40
    loop = int(data["loop"]) if "loop" in data else 0

    # Optional palette overrides: SS_PAL_0=R,G,B
    for i in range(palette.shape[0]):
        key = f"SS_PAL_{i}"
        if os.getenv(key):
            palette[i] = np.array(parse_color(os.getenv(key, ""), tuple(palette[i])), dtype=np.uint8)

    frames = []
    for t in range(frames_idx.shape[0]):
        idx = frames_idx[t]
        rgb = palette[idx]
        frames.append(Image.fromarray(rgb, mode="RGB"))

    frames[0].save(
        OUT_GIF,
        save_all=True,
        append_images=frames[1:],
        duration=duration,
        loop=loop,
        disposal=2,
    )
    print("gif", OUT_GIF)
    print("palette", [tuple(c.tolist()) for c in palette])


if __name__ == "__main__":
    main()
