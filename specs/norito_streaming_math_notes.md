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
Mirror symmetry is exact in the Q15 table. All folds are exact integers,
and the two Q15 passes round only the final 64-bit sum.
[Feig and Winograd, Fast Algorithms for the Discrete Cosine Transform](https://research.ibm.com/publications/fast-algorithms-for-the-discrete-cosine-transform)

In the floating-point table the reflected factors differ by at most
`1e-15` after applying the parity sign. Over a full-range 16-bit block,
replacing both factors perturbs a projection by less than `2.2e-9`.
The absolute-product sum is at most `64 * 32768 * 0.5² = 524288`.
The direct and folded evaluation errors together are bounded by
`524288 * (γ₆₆ + γ₁₈)`, where `γₙ = n*u/(1-n*u)` and `u = 2^-53`.
Within `2e-8` of a rounding midpoint the encoder evaluates the original
table in reference summation order.
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
weights to evaluate. Every retained AC coefficient keeps its reference
value for RDO.

The encoder accumulates sample sums and squared sums directly from the
source and collocated reference rows. A DC-only block can use these moments
without constructing either sample block. For the floating-point DC term,
`sum/8` can round to either neighboring integer at a midpoint. If both
integers map to the same quantized level the encoder uses that level
directly. Otherwise it evaluates the reference sum. Predicted blocks with
all zero levels retain their pixels in place. With RDO disabled, successive
such blocks share a bulk write of the standard DC-delta and end-marker
records. At Q0 the early copy requires every predicted pixel to match.

A common sparse block has nonzero AC levels only at the first horizontal
and vertical frequencies, each in `{-1, 0, 1}`. For these nine cases the
ordered entropy records depend only on those two levels, the repeating 8×8
block position, frame class and whether DC is zero. Within a segment the
recorder reuses the first emitted sequence for each such pattern. Its
histogram and context-switch counts add once per occurrence, and each block
still contributes its own ordered tokens. The recorder stores one key per
repeat and the literal spans between runs. Each pattern includes at least
one significance-RLE record, so
the entropy stream in that segment is the masked symbol byte of each record.
This projection distributes over concatenation. The encoder projects each
literal once, then copies its records and symbol bytes in block order to
produce the complete telemetry before returning the segment.

For unmapped contexts, blocks supported in the upper-left 3×3 share one
more sequence per tile and frame class. From zigzag order 26 onward, the
coefficient, preceding coefficient and every neighbor used in the context
are zero. Orders 25 and 26 have the same context inputs except for their
distinct final order bits, so a zero run ends at the boundary. Reusing
the suffix preserves record order. Its histogram and the switch from the
prefix add once per occurrence.

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
