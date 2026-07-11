# Kotodama exact numerics and declaration grammar (ABI V1)

This document is normative for the first Kotodama/IVM release. The keywords
`MUST`, `MUST NOT`, `SHOULD`, and `MAY` have their usual standards meaning.
There is no compatibility contract for the pre-release declaration grammar,
numeric types, pointer layouts, or numeric syscalls described as retired
below.

## Source-language surface

Kotodama uses type-first declarations consistently:

```text
seiyaku Counter {
    const int initial = 1;
    const int step = 2;

    state int value;

    hajimari() {
        value = initial;
    }

    kotoage fn increment() authorize("CanIncrement") {
        value = value + step;
    }

    view fn current() -> int {
        return value;
    }
}
```

The same order applies to local variables and parameters:

```text
let int count = 0;
fn add(int lhs, int rhs) -> int { return lhs + rhs; }
```

The old `name: Type` declaration form is invalid. The source names `i64`,
`u128`, `Amount`, `num`, and `float`, and suffixed numeric literals such as
`1i64`, `1u128`, and `1amt`, are invalid. Diagnostics MUST identify the retired
surface and show the type-first replacement.

V1 exposes three numeric types:

- `int`: a bounded signed integer with adaptive encoded width;
- `decimal`: a bounded exact base-10 value, not IEEE-754 floating point;
- `quantity`: a nominal, canonical, non-negative asset quantity.

The language does not define a primitive `money` type. Iroha assets may model
commodities, votes, rights, or money, so calling every balance “money” would be
incorrect. Applications MAY define nominal monetary types and policies over
`quantity`.

An unsuffixed whole-number literal has type `int` when no expected type exists.
It may be checked contextually as `decimal` or `quantity`; a contextual
`quantity` literal MUST be non-negative. A literal containing a decimal point
or decimal exponent has type `decimal` unless a `quantity` is expected.
Literal conversion is exact and occurs at compile time.
Literal mantissa and scale are normalized before the canonical scale bound is
checked. Thus a spelling with more than 28 fractional places is accepted only
when removing trailing decimal zeroes reduces its canonical scale to at most
28.

Integer literals MAY use decimal, `0x` hexadecimal, or `0b` binary spelling.
Decimal literals MAY use exact exponent notation. Digit separators are allowed
only between digits. `1.`, `.5`, repeated separators, and missing exponent
digits are invalid. Parsing MUST treat a leading sign and the following literal
as one range-checking operation so `-2^511` is accepted.

## Value domains

### `int`

An `int` is an integer `m` in the closed range:

```text
-2^511 <= m <= 2^511 - 1
```

This is a signed 512-bit two's-complement domain. “512-bit” never means a sign
plus a 512-bit magnitude.

### `decimal`

A `decimal` represents the mathematical value `m * 10^-s`, where:

```text
-2^511 <= m <= 2^511 - 1
0 <= s <= 28
```

Every value has exactly one canonical representation:

1. `(0, s)` normalizes to `(0, 0)`.
2. While `s > 0` and `m` is divisible by ten, replace `(m, s)` with
   `(m / 10, s - 1)`.

Canonicalization discards source-level significant-zero intent. Consequently
`1`, `1.0`, and `1.00` are the same numeric value and encode identically. A
separate application type is required when declared precision is part of the
business meaning.

### `quantity`

A `quantity` has the same canonical representation and scale bound as
`decimal`, with the additional invariant `m >= 0`. It is a distinct nominal
type. There is no implicit conversion from an arbitrary `decimal` or `int` to
`quantity`.

## Arithmetic semantics

Checked arithmetic computes the exact mathematical result using conceptually
unbounded intermediates, canonicalizes the result, and only then checks the
final V1 domain. An implementation's host bigint width, allocation strategy,
or temporary representation MUST NOT affect success, failure, or output.

The ordinary operator matrix is:

| Left | Operator | Right | Result | Semantics |
| --- | --- | --- | --- | --- |
| `int` | `+ - *` | `int` | `int` | exact, checked |
| `int` | `/ %` | `int` | `int` | quotient truncates toward zero |
| `decimal` | `+ - * /` | `decimal` | `decimal` | exact; `/` rejects an unrepresentable exact result |
| `int` | `+ - * /` and comparisons | `decimal` | `decimal`/`bool` | `int` promotes exactly |
| `decimal` | `+ - * /` and comparisons | `int` | `decimal`/`bool` | `int` promotes exactly |
| `quantity` | `+ -` | `quantity` | `quantity` | subtraction rejects underflow |
| `quantity` | `* /` | `decimal` | `quantity` | negative results reject |
| `quantity` | `/` | `quantity` | `decimal` | dimensionless exact ratio |

Other mixed arithmetic is invalid without an explicit named conversion.

For integer division, with nonzero `b`:

```text
q = trunc(a / b)
a = q * b + r
abs(r) < abs(b)
r has the sign of a unless r = 0
```

`min_int / -1` and `min_int % -1` fail with mantissa overflow because the
paired quotient is not representable. Division by zero has its own fault.

Decimal multiplication forms the exact product at the sum of input scales,
normalizes it, and rejects only when the canonical final scale remains above
28 or its final mantissa is out of range. It never rounds implicitly.

Exact division first reduces and classifies the denominator. A representable
quotient performs exactly one division at its proven minimum scale. A failure
performs no speculative output-scale attempts and distinguishes:

- `RepeatingDecimal`: the reduced denominator contains a prime factor other
  than two or five;
- `ExactDivisionScaleOverflow`: the expansion terminates but needs a canonical
  scale above 28.

Rounded division requires an output scale and one of these stable modes:

| Tag | Mode | Meaning |
| ---: | --- | --- |
| 0 | `toward_zero` | toward zero |
| 1 | `away_from_zero` | away from zero |
| 2 | `floor` | toward negative infinity |
| 3 | `ceil` | toward positive infinity |
| 4 | `nearest_even` | nearest, ties to an even mantissa |
| 5 | `nearest_away` | nearest, ties away from zero |
| 6 | `nearest_toward_zero` | nearest, ties toward zero |

Decimal-to-int conversion is exact by default. Named truncating and rounded
conversions are available. Quantity conversion is checked and explicit.

Checked negation, addition, subtraction, multiplication, division, and
remainder fail rather than wrap. The explicit integer wrapping operations use
modulo `2^512` and reinterpret the result in the signed domain. V1 does not
inherit an `i64` or `u128` modulus from retired source types.

V1 defines no source bitwise or shift operators. A future operation must first
specify its complete 512-bit two's-complement semantics, valid shift counts,
gas, and ABI surface; host-language bigint behavior is never inherited
implicitly.

## Canonical wire values

Numeric pointer payloads are complete, schema-bound, uncompressed Norito
frames. V1 permits no layout flags and no alignment padding.

The schema names and 16-byte schema hashes are:

| Type | Schema name | Hash (hex) |
| --- | --- | --- |
| `int` | `iroha.numeric.IntValueV1` | `07c039457363b9e1d36bbd31d93dec4a` |
| `decimal` | `iroha.numeric.DecimalValueV1` | `ba2ffed52e4d8ee16f17efefe1828524` |
| `quantity` | `iroha.numeric.QuantityValueV1` | `e4769984c81ce0e8b678f2eb06274ee3` |

The numeric body begins with a four-byte little-endian unsigned mantissa byte
length, followed by the minimal little-endian two's-complement bytes. Decimal
and quantity bodies append one scale byte. Integer bodies do not carry a scale.

Zero has length zero and no mantissa bytes. An empty mantissa is therefore the
only zero encoding. Redundant `0x00` or `0xff` sign extension is invalid. A
positive value needing a sign-preserving leading byte is valid only when the
complete minimal encoding remains at most 64 bytes.

With the fixed 40-byte Norito header, maximum frame sizes are 108 bytes for
`int` and 109 bytes for `decimal` and `quantity`. The pointer envelope adds a
seven-byte type/version/length header and a 32-byte payload hash, for maxima of
147, 148, and 148 bytes respectively.

Pointer type IDs are:

```text
0x0010  QuantityValueV1
0x0011  IntValueV1
0x0012  DecimalValueV1
```

Numeric equality, map-key hashing, and collection ordering operate on the
canonical mathematical value. SDKs MUST use arbitrary-precision integer or
exact-decimal representations and MUST NOT map these values to JavaScript
`number`, Java/Kotlin `double`, Swift `Double`, or another lossy host type.
Norito JSON and SDK-facing JSON render all three numeric domains as canonical
base-10 strings: `int` has no decimal point, while `decimal` and `quantity`
use the shortest non-exponent spelling implied by their canonical mantissa and
scale. JSON numeric tokens and alternate string spellings are rejected at a
typed numeric boundary.

## Syscalls and failures

ABI V1 contains the unconditional numeric syscall blocks:

```text
0x010100..0x010113  int
0x010120..0x01012f  decimal
0x010140..0x01014f  quantity
```

Typed exact-number JSON getters occupy `0x010160..0x010165`: `int`, `decimal`,
and `quantity`, followed by their direct-pointer variants. They accept only a
canonical base-10 JSON string. A JSON number token, exponent spelling, leading
plus or zero, negative quantity, removable fractional zero, or out-of-domain
string returns `Option::none`; it is never rounded or converted through a host
floating-point type.

The detailed signatures live in `crates/ivm/spec/syscalls.toml`. The retired
Numeric and Amount syscall blocks are not in the V1 allowlist. Every host MUST
implement the allowed blocks identically or reject an unknown number with
`VMError::UnknownSyscall`.

Stable numeric fault tags are:

```text
1 MantissaOverflow
2 ScaleOverflow
3 DivisionByZero
4 RepeatingDecimal
5 ExactDivisionScaleOverflow
6 InvalidScale
7 InexactConversion
8 NegativeQuantity
9 QuantityUnderflow
10 InvalidRoundingMode
11 InvalidFailureMode
12 ReservedRegisterNonZero
```

Fallible arithmetic accepts failure mode `0` (trap) or `1` (return the fault in
the status register). Conversions documented as recoverable always return a
status. Invalid pointer envelopes, versions, lengths, hashes, schemas, flags,
canonical encodings, scale pointers, rounding tags, failure modes, and required
zero registers have deterministic distinct validation behavior.

## Deterministic gas

Numeric syscalls use staged metering. They never reserve a worst-case quote and
never refund. Each phase is debited immediately before its bounded work begins;
an unaffordable phase performs no work and leaves earlier phase charges
consumed.

The successful aggregate identity is:

```text
gas = 16
    + canonical input envelope bytes
    + canonical output envelope bytes
    + 4 * logical_limb_work
```

There is no implicit minimum applied to `logical_limb_work`; operations whose
normative work is zero still pay the entry and envelope charges.

The constants are calibrated consensus weights, not host-cycle counts. The
entry weight `16` covers syscall dispatch, staged-context initialization,
bounded validation of at most four control/reserved registers, and completion
bookkeeping. Pointer traversal is excluded because every traversed envelope
byte is charged separately. The limb-work weight `4` budgets one logical
base-`2^64` work cell for operand access, the arithmetic/carry or trial step,
result access, and deterministic loop/control overhead. Multiplication counts
every input-cell product; division and normalization formulas count their
normalization, quotient-trial, remainder, and probe cells explicitly. Thus a
backend cannot make an uncharged algorithmic pass by selecting a different
bigint representation.

Release calibration uses `cargo bench -p ivm --bench gas_calibration`. The
`ivm-numeric-limb-cal` benchmark pins the formula's work denominator in every
benchmark ID for one through eight input limbs, products through sixteen
limbs, division/remainder, and scale-28 rounded division. For each supported
baseline hardware tier, maintainers compare median time per declared work cell
against the scalar IVM `ADD` baseline after subtracting harness overhead. The
rounded-up worst ratio, plus a minimum 25% safety margin, MUST remain no greater
than `4`; bounded dispatch/control overhead MUST remain no greater than `16`
baseline gas units. A failure requires increasing the constants, changing the
gas-formula version/hash, and regenerating gas goldens before release—it MUST
NOT be hidden by a hardware-specific implementation. The initial reference
run is pinned to Apple M1 Ultra (`Mac13,2`, arm64), Rust 1.93.1; release records
retain the Criterion output alongside the build artifacts and repeat the run
on the slowest supported tier.

The fixed entry charge includes bounded register-contract checks (required-zero
registers, failure mode, and rounding tag). It is not followed by hidden fixed
decode, divisor, or control surcharges. Strict numeric-frame validation adds
`ceil(frame_bytes / 8)` logical work units: the first unit covers the fixed
Norito header/schema/length checks, and the remaining units cover minimal
mantissa and decimal-domain canonicality. The frame bytes are still counted
exactly once as input-envelope bytes; checksum traversal is not charged a
second time.

Logical limbs are 64 bits, so an input has at most eight limbs. Arithmetic
width uses the bit length of the unsigned magnitude, with zero assigned one
limb. A sign-preserving byte in the canonical
two's-complement wire representation affects envelope-byte gas, not arithmetic
limb work. Let `B(d)` be the exact integer bit length of `10^d` and let:

```text
L(b) = max(1, ceil(b / 64))
P(d) = L(B(d))
S(0, d) = 1
S(b, 0) = L(b), for b > 0
S(b, d) = L(b + B(d)), for b > 0 and d > 0
A(b, 0) = 0
A(b, d) = L(b) * P(d), for d > 0
```

`B(d)` is pinned as an integer table for `d` in `0..=56`; gas computation never
uses floating-point logarithms. Addition, subtraction, and comparison charge
alignment multiplication plus the largest aligned width. Multiplication
charges the product of input limb widths.

The apparently tighter `L(b + B(d) - 1)` is not a valid consensus bound. For
example, a 61-bit value multiplied by ten can require 65 bits. The deliberately
conservative `S` above pins that boundary at two limbs on every implementation.

For long division with dividend width `n` and divisor width `d`:

```text
q = max(1, max(n, 1) - max(d, 1) + 1), with subtraction clamped at zero
division_work(n, d) = max(n, 1) + max(d, 1) + max(d, 1) * q
```

Exact division first charges the Euclidean reduction and denominator
classification steps actually begun. Once classification proves a terminating
result, it charges the single division at the proven minimum output scale
immediately before that division. Decimal normalization charges each
divide-by-ten probe immediately before the probe. Conceptual scales through
56, aligned or scale-adjusted widths through ten limbs, and multiplication
intermediates through sixteen limbs are included in golden vectors.

Pointer processing is ordered and charged as follows:

1. debit the fixed entry charge;
2. debit the seven-byte header charge, then validate readable provenance and
   read the header;
3. validate the hard length cap with checked arithmetic;
4. debit the 32 digest bytes and declared frame bytes, each exactly once;
5. snapshot exactly that range and validate its payload hash;
6. debit `4 * ceil(frame_bytes / 8)`, split into the frame-decode and
   canonical-validation phases, then validate the Norito frame, schema, flags,
   and canonical value;
7. debit each arithmetic/normalization phase before it begins;
8. determine the exact output envelope length and debit it;
9. allocate and write the output, then publish result registers.

Malformed inputs never cause allocation based on an uncapped length. Guest
memory used for execution is the same snapshot used for validation. Numeric
bytes may also incur transport/state decoding gas outside the syscall; that is
an intentional transport cost and is distinct from syscall marshalling.

## Error precedence and atomicity

Validation follows the phase order above. Within an operand, pointer
provenance/type/version/length precede payload hash, which precedes frame/schema
and canonical-value validation. Operands are validated in register order.
Only after every operand is valid are the scale pointer, rounding tag,
required-zero registers, and failure tag validated in register order. Invalid
rounding, failure, or required-zero controls are malformed-call traps with
their distinct fault tags; they are never converted to status-mode arithmetic
failures. A representationally valid scale pointer whose value is outside
`0..=28` remains the recoverable `InvalidScale` numeric fault. Control
validation precedes divisor validation; divisor validation precedes arithmetic.

If a phase is unaffordable, out-of-gas for that phase takes precedence over an
error discoverable only by performing the phase. A phase that has not begun is
not charged. No numeric failure writes an output envelope. A recoverable fault
writes only the result/status registers. Trapping and out-of-gas paths leave
numeric output allocation and durable state unchanged.

## Ledger boundary

Ledger balances remain asset-associated quantities, not generic decimals.
Every boundary uses an explicit checked conversion equivalent to:

```text
quantity::try_from_decimal(value)
decimal::from_quantity(value)
```

The conversion is exact and enforces the ledger's scale/range and any
asset-definition precision policy. Negative values, excess precision, and
out-of-range ledger results have stable errors. Generic decimal values are not
implicitly accepted by mint, burn, transfer, fee, or balance APIs.

## First-release activation and validation gates

ABI V1 has not previously been released as a compatibility contract. This
definition replaces every pre-release V1 numeric layout and syscall surface.
Old ABI hashes and artifacts are rejected before execution. Retired pointer IDs
remain reserved so they cannot be reinterpreted.

Merge and release require:

- full workspace format, build, test, and strict clippy gates;
- compiler-folding versus runtime differential tests;
- an independent exact-arithmetic reference implementation;
- cross-SDK canonical frame fixtures;
- encode/decode and malformed-frame fuzzing;
- quote-free staged gas/OOG tests at every phase;
- boundary vectors at every signed-byte and logical-limb transition;
- exact division, all signed rounding ties, wrapping endpoints, and
  multiplication/alignment intermediates above the input limb maximum;
- stale artifact/ABI-hash rejection and canonical numeric map-key tests;
- execution parity across supported architectures and bigint backends.
