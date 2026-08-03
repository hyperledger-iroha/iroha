"""Canonical T256 curve and field parameters for the Vega reference implementation.

These match the shipped library's ``T256HyraxEngine`` instantiation (T256 curve,
Keccak-256 transcript, Hyrax PCS). The T256 curve is a short-Weierstrass curve

    E : y^2 = x^3 - 3 x + B        over the base field F_p,

whose group order equals the scalar-field prime q (cofactor 1). Point coordinates
live in ``F_p`` (the *base* field); witnesses / challenges / exponents live in
``F_q`` (the *scalar* field).

Sources in the shipped library:
  * moduli               src/provider/pt256.rs (t256 impl_traits: scalar arg4, base arg5)
  * curve a = p-3, b     halo2curves-0.10.0/src/t256/curve.rs:57-70
  * generator (3, GY)    halo2curves-0.10.0/src/t256/curve.rs:42-54

This module holds the field/curve integer constants. The base field, the curve,
and the generator are implemented in pure Python in :mod:`pyvega.curve`; the
arithmetic core (:mod:`pyvega.field`, :mod:`pyvega.polys`) is integer-only.
"""

# Base field F_p — the field of point coordinates.
P = int("ffffffff0000000100000000000000017e72b42b30e7317793135661b1c4b117", 16)

# Scalar field F_q — the group order; field of witnesses, challenges, exponents.
Q = int("ffffffff00000001000000000000000000000000ffffffffffffffffffffffff", 16)

# Curve coefficients:  a = -3 (mod p),  b as published in curve.rs.
A = P - 3
B = int("b441071b12f4a0366fb552f8e21ed4ac36b06aceeb354224863e60f20219fc56", 16)

# Generator.
GX = 3
GY = int("5a6dd32df58708e64e97345cbe66600decd9d538a351bb3c30b4954925b1f02d", 16)

# Byte width of a serialized field element (both fields are < 2^256).
SCALAR_BYTES = 32
# Byte width of a compressed group element: 1 flag byte + 32-byte x-coordinate.
POINT_BYTES = 33

# Compressed-point flag bits (halo2curves CompressedFlagConfig::Extra).
FLAG_SIGN = 0x80  # y is odd  (sign = LSB of y's little-endian representation)
FLAG_IDENTITY = 0x40  # point at infinity
