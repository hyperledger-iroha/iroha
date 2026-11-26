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

The forward transform applies the 8-point basis along each axis. The Q15
implementation keeps the intermediate sums in 64-bit integers and rounds once
after both passes. The default floating-point implementation reevaluates a
coefficient in the reference summation order if it falls within `1e-8` of a
rounding midpoint. For 16-bit samples and basis factors of magnitude at most
one half, the absolute-product sum is bounded by 524,288. The combined
roundoff bound is at most `524288 * (γ₆₆ + γ₁₈) < 1e-8`, using
`γₙ = n*u/(1-n*u)` and binary64 unit roundoff `u = 2^-53`. Both paths
therefore preserve the reference integer coefficients before quantization.
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
The DC term keeps the reference rounding, including binary64 midpoints.
This is a sufficient condition adapted to the Norito matrices. Early
zero-coefficient detection from transform bounds is described by Ji et al.
[Early Determination of Zero-Quantized 8×8 DCT Coefficients](https://mcl.usc.edu/wp-content/uploads/2014/01/200912-Early-determination-of-zero-quantized-8%C3%978-DCT-coefficients.pdf)

When all quantized AC levels are zero, the ordered bundle records depend only
on the repeating 8×8 block position, frame class and whether DC is zero. The
recorder reuses the first emitted sequence for each of these 256 cases within
a segment. Its histogram and internal context-switch counts add once per
occurrence. Each block still contributes its own DC token and end marker.

A zero residual transforms to 64 zeros. When only the DC coefficient survives
quantization, its inverse transforms to the same sample at all 64 positions.
The scalar shortcuts preserve the basis, multiplication order, rounding and
clipping of their respective reference kernels.

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
