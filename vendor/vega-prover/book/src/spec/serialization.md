# Serialization and encodings

This chapter specifies the canonical byte string for a Vega proof object in the canonical T256 + Hyrax instantiation: the proof wire format. This is separate from the Fiat--Shamir transcript encodings in [Byte encodings and serialization](../building-blocks/encodings.md): transcript encodings decide which bytes are hashed into challenges, while wire serialization decides the bytes carried by the proof.

## Serialization domain

A proof is serialized by applying the rules in this chapter to the proof object. The reference implementation obtains the same bytes by calling `bincode::serialize(&proof)` with `bincode` 1.3.3.

The effective `bincode` configuration is:

```text
integer encoding: fixed-width integers (fixint), not varint
byte order:       little-endian for integer primitives
byte limit:       none
```

Equivalently, the emitted bytes match `DefaultOptions::new().with_little_endian().with_fixint_encoding()`. This equivalence concerns the produced bytes only; trailing-byte handling is a deserialization policy and does not change the serialized output. The verifier-key digest reuses this `bincode` configuration for most of its fields but encodes the constraint-system matrices through a separate manual byte routine; its object and digest domain are specified in [Verifier key and digest](./verifier-key.md).

An implementation does not need to use `bincode`. The following sections are the complete wire-format rules.

## Rust-shaped values

The proof object is a structured value whose fields are serialized recursively. Primitive integers use fixed widths; containers carry lengths where the table says they do.

| Value kind | Wire bytes |
| --- | --- |
| `u8`, `u16`, `u32`, `u64` | Fixed 1, 2, 4, or 8 bytes, little-endian. |
| `usize` | Encoded as a `u64`: 8 bytes, little-endian. |
| `bool` | One byte: `00` for false, `01` for true. |
| `Vec<T>` and other sequences/slices | An 8-byte little-endian `u64` element count, followed by each element in order. |
| `[T; N]` | The `N` elements in order, with no length prefix. |
| tuple | The fields in tuple order, with no tag, length prefix, or padding. |
| struct | The fields in declaration order, with no tag, length prefix, or padding. |
| `Option<T>` | One tag byte: `00` for `None`, `01` for `Some`; `Some` is followed by the encoding of `T`. |
| enum | A 4-byte little-endian `u32` variant index, 0-based in declaration order, followed by that variant's payload if it has one. |
| `String`, `&str` | An 8-byte little-endian `u64` byte length, followed by the UTF-8 bytes. |

Worked examples:

```text
usize value 5:
05 00 00 00 00 00 00 00

Vec<u8> value [aa, bb]:
02 00 00 00 00 00 00 00  aa bb

Option<u32> value Some(1):
01  01 00 00 00

Option<u32> value None:
00

enum unit variant at index 0:
00 00 00 00

enum variant at index 1 carrying u8 value 7:
01 00 00 00  07

String value "hi":
02 00 00 00 00 00 00 00  68 69
```

Struct serialization has no enclosing marker. A single-field struct serializes exactly as its field.

## Scalar-field elements

A scalar in \\(\mathbb{F}\\) serializes as the field element's canonical 32-byte `to_repr` output. For the scalar field in the canonical engine, this representation is little-endian.

```text
wire_scalar(s) = le32(integer representative of s)
```

Examples:

```text
scalar 1:
01 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00

scalar 258 = 0x0102:
02 01 00 00 00 00 00 00 00 00 00 00 00 00 00 00
00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
```

The transcript encoding of a scalar is different: [the transcript chapter](../building-blocks/encodings.md#scalar-field-elements) encodes scalar-field elements as 32 big-endian bytes. The transcript path reverses the scalar field's native little-endian representation; the wire path uses that native representation directly.

## Base-field elements

A base-field element serializes on the wire as its canonical 32-byte `to_repr` output. For the base field in the canonical engine this representation is little-endian, exactly as for the scalar field.

```text
wire_base(x) = le32(integer representative of x)
```

Both fields therefore serialize little-endian on the wire. The two fields differ only in the transcript path: the scalar field's transcript encoding is big-endian, while the base field's is little-endian ([the transcript chapter](../building-blocks/encodings.md#base-field-elements)). That asymmetry comes from the field backends' `to_bytes` methods — little-endian for the scalar field and big-endian for the base field — which the transcript reverses. The wire path uses `to_repr`, which is little-endian for both fields, so it is unaffected by that asymmetry.

A bare base-field element never appears as a standalone value in the proof object. The base field reaches the wire only as the x-coordinate inside a compressed group element, described next, where the point-compression routine writes it big-endian.

## Group elements

A group element in \\(\mathbb{G}\\) serializes to exactly 33 bytes in compressed form:

```text
wire_point(P) = flag_byte || x

flag_byte: 1 byte
x:         32 bytes, the affine x-coordinate in big-endian byte order
```

The x-coordinate occupies the 32 bytes following the flag byte, written as a 32-byte big-endian integer. This big-endian order is produced by the point-compression routine, which uses the base field's `to_bytes` method; it is not the little-endian `wire_base` encoding used for a standalone base-field element.

The flag byte uses two bits. All other bits are zero.

| Bit | Mask | Meaning |
| --- | --- | --- |
| 7 | `80` | Sign bit. Set when the affine y-coordinate is odd and the point is not the identity. Oddness is tested on the y-coordinate's canonical little-endian representation: the least-significant byte is odd. |
| 6 | `40` | Identity bit. Set when the point is the identity, also called the point at infinity. |

The identity serializes as the identity flag followed by a zero x-coordinate:

```text
40 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
```

The canonical generator has x-coordinate 3 and an odd y-coordinate. Its compressed wire encoding is:

```text
80 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 03
```

The transcript encoding of a group element is different: [the transcript chapter](../building-blocks/encodings.md#group-elements) uses the uncompressed 64-byte string `x || y`, where each coordinate is a 32-byte little-endian base-field element (defined for non-identity points; the transcript path does not encode the identity). The wire encoding uses the compressed 33-byte string `flag || x`, where `x` is big-endian and the identity is encoded explicitly. The same point therefore has two byte representations in two domains: transcript input and proof wire serialization.

## Commitment example

A `HyraxCommitment` is a vector of group elements wrapped in a single-field struct. Since single-field structs serialize exactly as their field, a commitment's wire bytes are the vector bytes:

```text
wire_commitment([P0, P1, ..., P(n-1)]) =
    u64_le(n) || wire_point(P0) || wire_point(P1) || ... || wire_point(P(n-1))
```

For a one-point commitment to the canonical generator, the bytes are an 8-byte count followed by the 33-byte compressed generator:

```text
01 00 00 00 00 00 00 00
80 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 03
```

This example shows how the primitive rules compose. The exact top-level proof fields and their declaration order are specified in [The proof object](./proof-object.md). The transcript schedule that consumes proof components is specified in [The transcript schedule](./transcript-schedule.md), and the transcript byte encodings themselves are specified in [Byte encodings and serialization](../building-blocks/encodings.md).
