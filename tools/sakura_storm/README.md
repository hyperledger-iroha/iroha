Sakura Storm v27 Model
======================

This directory keeps the v27 generation code and model inside the repo.

Files
- `extract_v27_model.py`: Extracts a compact model (palette + per-pixel indices) from a reference GIF.
- `gen_sakura_from_model.py`: Re-renders the GIF from the model (no reference GIF required).
- `v27_model.npz`: The extracted model used for generation.

Reproduce v27 (pixel-identical)
1) Extract the model (only needed if you update the reference GIF):
```
SS_REF_GIF=/path/to/sakura_storm_encoded_preview_logo_3rings_v27_fullwidth_katakana_lowdensity_bright.gif \
python3 extract_v27_model.py
```

2) Render from the model:
```
python3 gen_sakura_from_model.py
```
Output defaults to `/tmp/sakura_storm_viz/sakura_storm_v27_from_model.gif` if that directory exists.

Palette tweaks
Use `SS_PAL_<index>=R,G,B` to recolor without changing structure. Example:
```
SS_PAL_7=255,220,245 SS_PAL_8=255,220,245 \
SS_PAL_4=60,34,52 SS_PAL_5=60,34,52 SS_PAL_6=60,34,52 \
python3 gen_sakura_from_model.py
```

Default palette indices (from v27):
- 0: (11, 7, 12)   bg
- 1: (30, 16, 25)
- 2: (36, 18, 28)
- 3: (43, 20, 32)  logo shades
- 4: (62, 36, 50)
- 5: (68, 40, 54)
- 6: (69, 40, 54)  dim data
- 7: (255, 233, 246)
- 8: (255, 234, 246) bright data
- 9: (255, 246, 252) ring bright
