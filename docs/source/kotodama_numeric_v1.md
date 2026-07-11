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

Integer literals MAY use decimal, `0x` hexadecimal, or `0b` binary spelling.
Decimal literals MAY use exact exponent notation. Digit separators are allowed
only between digits. `1.`, `.5`, repeated separators, and missing exponent
digits are invalid. Parsing MUST treat a leading sign and the following literal
as one range-checking operation so `-2^4095` is accepted.

## Value domains

### `int`

An `int` is an integer `m` in the closed range:

```text
-2^4095 <= m <= 2^4095 - 1
```

This is a signed 4,096-bit two's-complement domain. “4,096-bit” never means a
sign plus a 4,096-bit magnitude.

### `decimal`

A `decimal` represents the mathematical value `m * 10^-s`, where:

```text
-2^4095 <= m <= 2^4095 - 1
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

Exact division tries canonical output scales `0..=28` in ascending order. If no
attempt is exact, it distinguishes:

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
modulo `2^4096` and reinterpret the result in the signed domain. V1 does not
inherit an `i64` or `u128` modulus from retired source types.

V1 defines no source bitwise or shift operators. A future operation must first
specify its complete 4,096-bit two's-complement semantics, valid shift counts,
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
complete minimal encoding remains at most 512 bytes.

With the fixed 40-byte Norito header, maximum frame sizes are 556 bytes for
`int` and 557 bytes for `decimal` and `quantity`. The pointer envelope adds a
seven-byte type/version/length header and a 32-byte payload hash, for maxima of
595, 596, and 596 bytes respectively.

Pointer type IDs are:

```text
0x0010  retired; permanently reserved and rejected
0x0011  IntValueV1
0x0012  DecimalValueV1
0x0013  QuantityValueV1
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

Logical limbs are 64 bits. Arithmetic width uses the bit length of the unsigned
magnitude, with zero assigned one limb. A sign-preserving byte in the canonical
two's-complement wire representation affects envelope-byte gas, not arithmetic
limb work. Let `B(d)` be the exact integer bit length of `10^d` and let:

```text
L(b) = max(1, ceil(b / 64))
P(d) = L(B(d))
S(0, d) = 1
S(b, d) = L(b + B(d) - 1), for b > 0
A(b, 0) = 0
A(b, d) = L(b) * P(d), for d > 0
```

`B(d)` is pinned as an integer table for `d` in `0..=56`; gas computation never
uses floating-point logarithms. Addition, subtraction, and comparison charge
alignment multiplication plus the largest aligned width. Multiplication
charges the product of input limb widths.

For long division with dividend width `n` and divisor width `d`:

```text
q = max(1, max(n, 1) - max(d, 1) + 1), with subtraction clamped at zero
division_work(n, d) = max(n, 1) + max(d, 1) + max(d, 1) * q
```

Each exact output-scale attempt is charged separately immediately before it
begins. Failure classification charges each Euclidean remainder,
reduced-denominator division, divisibility probe, and successful factor
division actually begun. Decimal normalization charges each divide-by-ten
probe immediately before the probe. Conceptual scales through 56 and widths
beyond the 64-limb input maximum are included in golden vectors.

Pointer processing is ordered and charged as follows:

1. debit the fixed entry charge;
2. debit the seven-byte header charge, then validate readable provenance and
   read the header;
3. validate the hard length cap with checked arithmetic;
4. debit the remaining declared envelope byte charge;
5. snapshot exactly that range;
6. validate the payload hash, Norito frame, schema, flags, and canonical value;
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
Scale and rounding validation precede divisor validation; divisor validation
precedes arithmetic.

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
