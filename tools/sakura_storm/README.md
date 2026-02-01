Sakura Storm v27 Model
======================

This directory keeps the v27 generation code and model inside the repo.

Files
- `v27_params.py`: Canonical constants/seeds for v27 reproduction (includes Iroha `KATAKANA` with archaic extras + v27 `KATAKANA_V27`).
- `repro_v27_exact.py`: One-shot procedural + symbol replay reproduction with diffs.
- `repro_v27_iroha.py`: One-shot Iroha/archaic katakana procedural + symbol replay with diffs.
- `repro_v27_katabox.py`: One-shot katakana overlay (boxes + diff glyphs) pipeline.
- `export_v27_manifest.py`: Writes a JSON manifest of v27 parameters.
- `extract_v27_model.py`: Extracts a compact model (palette + per-pixel indices) from a reference GIF.
- `gen_sakura_from_model.py`: Re-renders the GIF from the model (no reference GIF required).
- `analysis_v27_fit.py`: Analyzes the reference GIF to infer grid/glyph parameters.
- `analysis_v27_glyph_fit.py`: Fits katakana font/size/threshold against dynamic cell masks.
- `analysis_v27_static_masks.py`: Summarizes static cell mask shapes and best-fit box padding.
- `gen_sakura_v27_procedural.py`: Procedural generator (no model) using inferred parameters.
- `extract_v27_symbols.py`: Extracts per-cell glyph indices + bits from the reference GIF, plus
  a 15‑glyph mask palette and per‑cell allowed‑mask (logo clipping) so the replay stays exact.
- `gen_sakura_v27_symbols.py`: Renders a GIF from the extracted symbol stream (exact match using glyph masks + allowed masks + static ring layer).
- `compare_v27.py`: Quick diff metrics between a candidate and the reference.
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
Use `SS_PAL_<index>=R,G,B` to recolor without changing structure, or set
`SS_PALETTE_PRESET=v27|v26_preview|defi_crimson|v26_crimson|defi_crimson_bold|v26_ink|v26_teal|v26_mono|logo_forward` for quick swaps. Example:
```
SS_PAL_7=255,220,245 SS_PAL_8=255,220,245 \
SS_PAL_4=60,34,52 SS_PAL_5=60,34,52 SS_PAL_6=60,34,52 \
python3 gen_sakura_from_model.py
```
Quick preset example:
```
SS_PALETTE_PRESET=defi_crimson python3 gen_sakura_from_model.py
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

One-shot reproduction (procedural + symbol replay + diffs)
```
SS_REF_GIF=/path/to/sakura_storm_encoded_preview_logo_3rings_v27_fullwidth_katakana_lowdensity_bright.gif \
python3 repro_v27_exact.py
```
Set `SS_OUT_DIR=/path` to choose an output directory. Use `SS_DO_SYMBOLS=0` to skip
symbol extraction/replay. (`repro_v27_exact.py` pins `SS_KATAKANA_MODE=v27`.)

One-shot Iroha/archaic variant (procedural + symbol replay + diffs)
```
python3 repro_v27_iroha.py
```

One-shot katakana box overlay (uses clean base → extract → overlay replay)
```
python3 repro_v27_katabox.py
```
Set `SS_PALETTE_PRESET=...` to choose the katabox palette; outputs are tagged with the preset name.
Set `SS_PALETTE_PRESETS=a,b,c` to generate multiple presets in one run.

Export v27 manifest
```
python3 export_v27_manifest.py
```

Procedural generator (reverse-engineering)
```
SS_REF_GIF=/path/to/sakura_storm_encoded_preview_logo_3rings_v27_fullwidth_katakana_lowdensity_bright.gif \
python3 analysis_v27_fit.py

python3 gen_sakura_v27_procedural.py
python3 compare_v27.py
```
Extract + replay symbol stream:
```
SS_REF_GIF=/path/to/sakura_storm_encoded_preview_logo_3rings_v27_fullwidth_katakana_lowdensity_bright.gif \
python3 extract_v27_symbols.py

python3 gen_sakura_v27_symbols.py
```
Notes:
- Cells are row‑major 32×32 at 16px, with centers inside radius 243.0.
- Outer mask is a pixel circle at radius 245.0 (applied after drawing).
- The ring is a static layer drawn before data; data glyphs occlude ring pixels.
- The logo is also drawn before data (no data clipping); it is derived by
  thresholding the resized logo alpha into three shades (>=185, >=229, >=247).
- Data bits are RA‑encoded (rate 0.25, row‑weight 4, seed 2027) from a
  source RNG seeded with 1234; glyph indices use random.Random(1337).
Defaults now mirror the reverse‑engineered layout (32x16 grid, outer radius mask, and a low‑density glyph pool).
Useful knobs:
- `SS_ACTIVE_BANDS=""` (defaults off; ring holes come from the ring overlay)
- `SS_MASK_LOGO=1` to exclude logo cells (v27 keeps data over the logo)
- `SS_GLYPH_POOL_MODE=auto|single_low|single_low_extreme|lowhigh|all`
- `SS_KATAKANA_MODE=v27|iroha` (default `v27`; `iroha` uses the Iroha + archaic list)
- `SS_PALETTE_PRESET=v27|v26_preview|defi_crimson|v26_crimson|defi_crimson_bold|v26_ink|v26_teal|v26_mono|logo_forward` (optional recolor for procedural/symbol renders)
- `SS_ACTIVE_BANDS=inner-outer[,inner-outer...]` keeps data to annular bands (katabox default `0-243` to mimic v26 preview density).
- `SS_DYNAMIC_RADIUS=0` (default) keeps all cells dynamic; set >0 to split dynamic vs static regions
- `SS_DYNAMIC_POOL=low` / `SS_STATIC_POOL=high` to tune density split when enabled
- `SS_FONT_INDEX=0` and `SS_GLYPH_SIZE=15` (matches dynamic glyphs)
- `SS_SOURCE_SEED=1234` and `SS_RA_SEED=2027` to reproduce the exact bitstream
Katakana overlay (v26-style boxes + diff glyphs)
- `SS_KATAKANA_OVERLAY=1` to draw katakana on top of data cells.
- `SS_KATAKANA_BOXES=1` draws a box behind the glyph; set `SS_KATAKANA_TEXT_ON=0,0,0` to flip text to black.
- `SS_KATAKANA_DIFF=1` swaps glyphs on bit flips (difference mask effect).
- `SS_KATAKANA_NONDATA=1` draws overlay on non-data cells instead of data cells.
- `SS_KATAKANA_BOX_ON=R,G,B` / `SS_KATAKANA_BOX_OFF=R,G,B` customize box colors.
- `SS_KATAKANA_TEXT_ON=R,G,B` / `SS_KATAKANA_TEXT_OFF=R,G,B` customize text colors.
- `SS_KATAKANA_ALPHA`, `SS_KATAKANA_SCALE`, `SS_KATAKANA_BOX_ALPHA`, `SS_KATAKANA_BOX_SCALE` tune opacity/size.
- `SS_RING_DIM_ANGLES=0,120,240` and `SS_RING_DIM_TOL=4` tune the orientation dot pattern.
Logo emphasis
- `SS_LOGO_SCALE` controls logo size (v27 default 0.318; katabox default 0.12).
- Katabox defaults match the v26 preview grid (`SS_GRID_N=32`, `SS_CELL_SIZE=16`, `SS_GRID_OFFSET=0`) with slimmer glyphs (`SS_GLYPH_SIZE=10`, `SS_GLYPH_THRESH=140`) to match v26 density.
- `SS_LOGO_THRESH_1/2/3` adjust logo shade thresholds (lower = bolder logo).
- `SS_LOGO_OVERLAY=1` draws the logo on top of data (for visibility).
- `SS_LOGO_OVERLAY_ALPHA` controls overlay strength (0-255).
- `SS_LOGO_OVERLAY_COLOR=R,G,B` forces a bright logo tint.
- `SS_LOGO_OVERLAY_POST=1` applies the logo after palette quantization (best for bright tints).
- `SS_LOGO_ALPHA=0` removes the shaded logo while keeping the mask (katabox default; gives a v26-style blank logo hole).
- `SS_LOGO_MASK_REF=/path/to/ref.gif` and `SS_LOGO_MASK_COLORS=R,G,B;R,G,B;...` can pull the logo hole directly from a reference image (katabox defaults to the v26 preview mask if it exists).
- `SS_KATAKANA_OUTSIDE_LOGO=1` keeps katakana boxes off the logo area (logo mask is computed from the logo alpha even if overlay is off).
- `SS_MASK_LOGO=1` removes data glyphs under the logo (best for visibility).
Tip: for symbol replay with custom ring orientation, set `SS_RING_OVERRIDE=1`.
Tip: `SS_FORCE_PALETTE=1` forces palette quantization in symbol replay (useful after overlays).
Example:
```
SS_KATAKANA_MODE=iroha SS_PALETTE_PRESET=defi_crimson_bold \
SS_KATAKANA_OVERLAY=1 SS_KATAKANA_BOXES=1 SS_KATAKANA_DIFF=1 \
SS_KATAKANA_TEXT_ON=0,0,0 SS_KATAKANA_TEXT_OFF=255,230,230 \
SS_KATAKANA_BOX_ON=255,70,70 SS_KATAKANA_BOX_OFF=30,12,16 \
SS_KATAKANA_ALPHA=140 SS_KATAKANA_BOX_ALPHA=140 SS_KATAKANA_SCALE=0.85 \
SS_RING_DIM_ANGLES=0,120,240 \
python3 gen_sakura_v27_procedural.py
```
Tip: Use the same overlay knobs with `gen_sakura_v27_symbols.py` to add katakana on top of a
pre-extracted symbol stream (recommended when you want to keep extraction clean).

Iroha/archaic katakana variant (procedural + symbol replay)
```
SS_KATAKANA_MODE=iroha \
SS_OUT_GIF=/tmp/sakura_storm_viz/sakura_storm_v27_procedural_iroha.gif \
python3 gen_sakura_v27_procedural.py

SS_KATAKANA_MODE=iroha \
SS_REF_GIF=/tmp/sakura_storm_viz/sakura_storm_v27_procedural_iroha.gif \
SS_OUT_NPZ=/tmp/sakura_storm_viz/v27_symbols_iroha.npz \
python3 extract_v27_symbols.py

SS_SYMBOLS_NPZ=/tmp/sakura_storm_viz/v27_symbols_iroha.npz \
SS_OUT_GIF=/tmp/sakura_storm_viz/sakura_storm_v27_from_symbols_iroha.gif \
python3 gen_sakura_v27_symbols.py
```
Tip: add `SS_PALETTE_PRESET=defi_crimson` for a v26‑style crimson recolor.
