# Norito Streaming Math Roadmap (Non-Normative)

This note describes the NSC baseline encoder and potential extensions. Encoder
decisions must preserve determinism across hardware.

## Encoder decisions

**Video.** The inner block stream carries a 16-bit DC delta, one 24-bit end marker
and one 24-bit run/level record per nonzero AC coefficient. All 63 AC positions
fit in one run byte. The encoder chooses zero at an exact midpoint between zero
and the first nonzero level. This saves one record at equal transform-domain
squared error. Predicted frames use reconstructed pixels, so the size and
distortion of later frames must be measured after decoding the complete segment.

Each segment starts with an intra frame. The encoder reconstructs a plane only
when another frame in that segment can use its pixels for prediction. At nonzero
quantizers the encoder compares the source with the residual from the previous
reconstruction before choosing the next frame type. It divides each padded plane
into at most 4×4
regions of 8×8 blocks. The central block in each region contributes
`64 * sum(x²) - sum(x)²`, weighted by the number of blocks it represents.
The fixed-width DC field makes a uniform brightness offset cheap in either
mode. The encoder refreshes all planes when their combined intra cost is
strictly below 80% of the predicted cost. Quantizer zero keeps temporal
prediction because smaller nonzero coefficients still occupy a full token.
With the in-loop luma filter enabled, both sides use the filtered
reconstruction as their next reference. Comparing intra and inter costs for
scene changes follows the approach used in x265. The block sampling, cost and
threshold here are specific to Norito.
[x265 scene-cut analysis](https://github.com/videolan/x265/blob/master/source/encoder/slicetype.cpp)

The forward DCT pairs samples reflected about each axis. Let `a,b,c,d` be
the top-left, top-right, bottom-left and bottom-right samples of one such
quartet. For frequency indices `u,v`, an 8×8 projection uses a 4×4 array
with entries `a + (-1)^v b + (-1)^u c + (-1)^(u+v) d`. The encoder forms
the four arrays once and evaluates each frequency from its parity group.
The encoder folds directly from the source and collocated prediction rows. At
nonzero quantizers the band moments already supply the DC sum. The 16 weights
of each projection are the outer product of the vertical and
horizontal basis factors. In the Q15 transform these products are exact Q30
integers. A signed 64-bit dot product accumulates all terms before rounding.
Scalar code and AArch64 NEON use this same integer sum.
[Feig and Winograd, Fast Algorithms for the Discrete Cosine Transform](https://research.ibm.com/publications/fast-algorithms-for-the-discrete-cosine-transform)

For the binary64 transform, each outer-product weight is rounded to the
nearest multiple of 2⁻³². For 16-bit input, the sum of absolute folded
samples in a group is at most `64 × 32768 = 2²¹`, so weight quantization
contributes at most 2²⁰ units in the Q32 accumulator. The tabulated mirror
pairs differ by at most `1e-15` after applying the parity sign. Replacing
both factors perturbs a full projection by less than `2.2e-9`. The
absolute-product sum is at most `64 × 32768 × 0.5² = 524288`. The error
from forming the binary64 weight products and from direct reference evaluation
is at most `524288 × (γ₁ + γ₆₆)`, where `γₙ = n·u/(1−n·u)` and
`u = 2⁻⁵³`. Together these last errors fit within 128 Q32 units. When
the integer sum is within `2²⁰ + 128` units of a half-integer, the
encoder evaluates that coefficient in the original binary64 order.
[Higham, Accuracy and Stability of Numerical Algorithms, ch. 3](https://eprints.maths.manchester.ac.uk/238/)

At nonzero quantizers the encoder can prove that all 63 AC levels will be zero
before computing the transform. For block samples `x`, it computes
`E = 64 * sum(x²) - sum(x)²` in signed 64-bit arithmetic. The norm of the
mean-centered block is `sqrt(E/64)`. The squared norm of every 1-D basis row
is at most `65/64`, so the norm of every 2-D basis (an outer product) is also
at most `65/64`. Cauchy–Schwarz bounds each centered projection by their
product. With `H` equal to half the smallest AC step rounded down, a block
qualifies when `E <= 64 * floor((H - 2) * 64 / 65)²` and `H >= 2`.
The two-unit margin covers the fixed-point signed rounding error (less than
1.5 units), or the floating-point rounding and tiny nonzero AC row sums.
For a qualifying block the encoder quantizes the DC term with the reference
rounding, including binary64 midpoints. It writes the DC delta and end marker
directly. When a later frame needs the reconstruction, each pixel receives
the same inverse DC contribution before prediction and clipping.
This is a sufficient condition adapted to the Norito matrices. Early
zero-coefficient detection from transform bounds is described by Ji et al.
[Early Determination of Zero-Quantized 8×8 DCT Coefficients](https://mcl.usc.edu/wp-content/uploads/2014/01/200912-Early-determination-of-zero-quantized-8%C3%978-DCT-coefficients.pdf)

The same energy bounds individual frequencies when the smallest AC step is
insufficient. With `U = isqrt(E) + 1`, the encoder bounds each rounded AC
magnitude by `B = ceil(65 * U / 512) + 2`. A matrix weight `w` with
`w * quantizer > 2B` guarantees a zero level. The encoder skips these
projections. Within a reflection group, let `L` be the sum of absolute
folded samples. Each 2-D basis factor has magnitude at most one quarter,
so `ceil(L/4) + 2` gives a second bound for that group's AC coefficients.
In the even/even group the encoder subtracts the integer mean of the 16
folded entries for this bound, since a constant contributes only to DC up to
the table error above. The tighter of the two bounds determines which
weights to evaluate. Each group's projections are stored in ascending weight
order. The first weight above the threshold ends that group. The default
path quantizes each visited projection immediately. For step `s` and integer
coefficient magnitude `m`, the first nonzero level has magnitude one when
`floor(s/2) < m < 2s − floor(s/2)`. The encoder restores its sign directly.
RDO receives the reference coefficient before selecting levels.

Each block needs two integer moments, the sum of the residuals and their
squared sum. The encoder computes one band of eight scanlines before writing
back its collocated predictors. On AArch64 with SIMD and dot-product support,
a 16-byte load spans two adjacent blocks. The dot product of pixel bytes
with ones gives their sum. Subtracting the predictor sum gives the signed
residual sum. The dot product of the absolute differences with themselves
gives the squared sum. Each four-byte lane belongs entirely to one block.
The final pairwise addition produces both block results. An unpaired block
uses the scalar calculation.
[Arm, integer dot products with Rust SIMD intrinsics](https://learn.arm.com/learning-paths/cross-platform/simd-on-rust/simd-on-rust-part2/)

For the binary64 DC coefficient, let `S` be the signed residual sum and
`d` the DC quantization step. Compute the biased magnitude
`n = floor((|S|+4)/8) + floor(d/2)`. The quotient `floor(n/d)` is exactly
`floor(n * ceil(2^32/d) / 2^32)`. Even for full-range 16-bit samples,
`n ≤ 262144 + 2040` and `d ≤ 4080`, so the reciprocal error is less than
`1/d` in the quotient. At a half-integer DC, rounding the magnitude down
can change the level only when `n` is divisible by `d`. The encoder uses
the reference summation order at those boundaries and otherwise restores
the sign to the integer quotient. Predicted blocks with
all zero levels retain their pixels in place. With RDO disabled, successive
such blocks share a bulk write of the standard DC-delta and end-marker
records. At Q0 the early copy requires every predicted pixel to match. Reconstruction uses the collocated
pixel before overwriting it. The encoder can retain the three plane buffers
between segments because the first Intra frame ignores prior pixels and
writes every sample needed by the following frame.

When each of the first five zigzag AC levels lies in `{-1, 0, 1}` and
all higher AC levels are zero, five base-three digits identify one of 243
patterns. Together with the 8×8 tile position, frame class and DC significance,
these digits determine the block's ordered entropy records. The recorder
counts repeated patterns by multiplicity and emits each block's ordered
tokens. For unmapped contexts, changing the signs of occupied ternary AC
positions preserves the context sequence, symbol widths and zero runs. An
encoder therefore keeps one prefix per five-bit occupancy mask, tile, frame
class and DC significance. It flips the sign bit at the affected nonzero
records when it copies a prefix. Per-segment histograms count the resulting
signed records. The table has at most 32 × 256 entries of at most 25 records. The same support bound limits the run/level writer's zigzag scan.

Each repeated pattern contains a significance-RLE record, so the segment's
entropy stream is the masked symbol byte of each record. The recorder projects
literal records once and expands repeats in block order. An aligned 63-record
span can retain both arrays from the preceding report when its canonical key
matches. When just the neutral suffix matches at the same offset, only the
prefix needs to be written. The canonical keys are independent of the order
in which patterns first appear in either segment.

For unmapped contexts, blocks supported in the upper-left 3×3 share one
more sequence per tile and frame class. From zigzag order 26 onward, the
coefficient, preceding coefficient and every neighbor used in the context
are zero. Orders 25 and 26 have the same context inputs except for their
distinct final order bits, so a zero run ends at the boundary. Reusing
the suffix preserves record order. Its histogram and the switch from the
prefix add once per occurrence. When only DC and the first horizontal and
vertical frequencies are present, orders 11 and 12 are already neutral and
differ only in order. In this case the common suffix starts at order 12.
Eight successive predicted zero blocks in a raster row visit each tile
column once, so their template and end-marker counts can be added by cycle.

For the context hash, the encoder tabulates the FNV-1 prefix through frame
class modulo 2¹⁶. Multiplication and exclusive-or commute with truncation
to the low 16 bits. The remaining tile, significance and order fields
therefore produce the same context ID. Neighbor counts use eight unsigned
byte lanes per row. Adding the left, upper-left, upper and upper-right
occupancies gives at most four in a lane, which prevents carries between
pixels. A one-zero run of the configured width contributes one record, a
known bit length and one count for symbol 1. Its multiplicity is sufficient
to reconstruct those fields in each context histogram. For a context with
`N` records, the observed outgoing switch count is `N − S − E − U`, where
`S` counts consecutive records in that same context, `E` counts block
terminators attributed to it, and `U` is one if its record ends an
unterminated stream. Explicit flush reasons are added separately.
[FNV-1 recurrence, RFC 9923 §2](https://www.rfc-editor.org/rfc/rfc9923.html#section-2)

A zero residual transforms to 64 zeros. When only the DC coefficient survives
quantization, its inverse transforms to the same sample at all 64 positions.
For mixed blocks, each nonzero transform coefficient contributes a rank-one
term to the spatial block: `C[u,v] * B[u,x] * B[v,y]` at pixel `(x,y)`.
With `k` nonzero coefficients, reconstruction adds `64k` terms instead of
4,096. Visiting the coefficients in raster order preserves each pixel’s
multiplication and addition order, including Q15 saturation before the final
rounding and clipping. Choi, Lee and Jang also exploit coefficient sparsity
in their [zero coefficient-aware inverse DCT](https://doi.org/10.1109/TCE.2010.5606332).

For enabled luma RDO modes, the existing lambda schedule sets a nonzero-token
budget by counting levels whose squared quantized magnitude exceeds the price of
one token. For each candidate with source transform value `c` and dequantized
value `r`, retaining the level saves `r * (2*c - r)` in squared error. Selecting
the largest gains minimizes transform error within that block budget. Lower
raster slots win ties. The selector uses an integer comparison and a fixed
63-entry scratch array. The AC decisions borrow the source/reconstruction
distortion model from encoder research and use Norito’s fixed-width block record costs before frame compression.
[Wieckowski et al., EUSIPCO 2024](https://eurasip.org/Proceedings/Eusipco/Eusipco2024/pdfs/0000516.pdf)
and [Nalci et al., AV2 transform and entropy coding, §III-H](https://arxiv.org/html/2601.02712v2#S3.SS8)

**Audio.** The native encoder evaluates the peak-delta step and candidates at
7/8, 3/4 and 1/2 of that step. It reconstructs the samples of the current
channel with the decoder's predictor and minimizes their summed squared PCM
error. The peak step is the first candidate, so the result has at most its
error at the same packet size. This applies the analysis-by-synthesis approach
used in ADPCM encoders to the single step already carried in a Norito frame.
[ADPCM-XQ implementation](https://github.com/dbry/adpcm-xq/blob/master/adpcm-lib.c)

## Video Coding

- **Integer motion compensation**: add deterministic block matching
  (integer-pel, diamond/hex search) with bounded search windows to reduce
  residual energy without introducing non-determinism.
- **Rate-model RDO for rANS**: if rANS symbols become the delivered block payload,
  price their context-dependent costs from the negotiated tables. Today the
  inner plane stream uses fixed-width RLE records and rANS byte counts belong to telemetry.
- **Adaptive quantization**: scale the quant matrix per block using block
  variance/contrast sensitivity in integer math to target perceptual quality
  without changing the bitstream grammar.
- **Deterministic post-filter**: add an in-loop deblock/dering filter with fixed
  coefficients (no floating point) to reduce ringing at low bitrates.

## Audio Coding

- **LPC + Rice residuals**: deterministic prediction + Rice coding can
  outperform ADPCM while keeping the implementation small and fixed-point.
- **Fixed-point MDCT**: a short-window MDCT with coarse psychoacoustic weighting
  yields better quality than ADPCM and remains hardware-stable when implemented
  with integer arithmetic.

## Acceleration

- **SIMD/GPU**: add SIMD kernels for the fixed-point DCT/IDCT and bundle rANS
  paths behind feature flags to improve throughput while preserving bit-exact
  output.

## Determinism Requirements

- No data-dependent parallel reductions.
- Use integer arithmetic for fixed-point kernels and new coding decisions.
- Feature flags must preserve deterministic fallbacks.
