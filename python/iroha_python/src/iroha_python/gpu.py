"""CUDA acceleration helpers exposed through the `_crypto` extension module."""

from __future__ import annotations

from typing import Optional, Sequence, Tuple

from ._native import load_crypto_extension

_crypto = load_crypto_extension()

__all__ = [
    "cuda_available",
    "cuda_disabled",
    "poseidon2_cuda",
    "poseidon2_cuda_many",
    "poseidon6_cuda",
    "poseidon6_cuda_many",
    "bn254_add_cuda",
    "bn254_add_cuda_many",
    "bn254_sub_cuda",
    "bn254_sub_cuda_many",
    "bn254_mul_cuda",
    "bn254_mul_cuda_many",
]


def cuda_available() -> bool:
    """Return ``True`` when the CUDA backend initialised successfully."""

    return bool(_crypto.cuda_available())


def cuda_disabled() -> bool:
    """Return ``True`` when the CUDA backend has been disabled after a failure."""

    return bool(_crypto.cuda_disabled())


def poseidon2_cuda(a: int, b: int) -> Optional[int]:
    """Execute the Poseidon2 permutation via CUDA when available.

    Returns ``None`` when CUDA support is unavailable or the backend has been disabled.
    """

    result = _crypto.poseidon2_cuda(int(a), int(b))
    return int(result) if result is not None else None


def poseidon2_cuda_many(pairs: Sequence[Sequence[int]]) -> Optional[Tuple[int, ...]]:
    """Execute multiple Poseidon2 permutations via CUDA when available.

    ``pairs`` must be an iterable of ``(a, b)`` tuples. Returns ``None`` when CUDA support is
    unavailable or disabled. When successful, the result is a tuple containing the hashed field
    elements represented as Python integers.
    """

    materialised = []
    for pair in pairs:
        if len(pair) != 2:
            raise ValueError("poseidon2_cuda_many expects 2-tuples")
        materialised.append((int(pair[0]), int(pair[1])))

    result = _crypto.poseidon2_cuda_many(tuple(materialised))
    if result is None:
        return None
    return tuple(int(value) for value in result)


def poseidon6_cuda(inputs: Sequence[int]) -> Optional[int]:
    """Execute the Poseidon6 permutation via CUDA when available.

    The ``inputs`` sequence must contain exactly six integers representing field elements.
    Returns ``None`` when CUDA support is unavailable or the backend has been disabled.
    """

    if len(inputs) != 6:
        raise ValueError("poseidon6_cuda expects exactly six input elements")
    result = _crypto.poseidon6_cuda(tuple(int(value) for value in inputs))
    return int(result) if result is not None else None


def poseidon6_cuda_many(inputs: Sequence[Sequence[int]]) -> Optional[Tuple[int, ...]]:
    """Execute multiple Poseidon6 permutations via CUDA when available.

    Each inner sequence must contain six integers. Returns ``None`` when CUDA support is unavailable
    or disabled. When successful, the result is a tuple containing the hashed field elements.
    """

    materialised = []
    for entry in inputs:
        if len(entry) != 6:
            raise ValueError("poseidon6_cuda_many expects sequences of length six")
        materialised.append(tuple(int(value) for value in entry))

    result = _crypto.poseidon6_cuda_many(tuple(materialised))
    if result is None:
        return None
    return tuple(int(value) for value in result)


def _expect_field_elem(elem: Sequence[int], context: str) -> Tuple[int, int, int, int]:
    if len(elem) != 4:
        raise ValueError(f"{context} expects four 64-bit limbs")
    mask = (1 << 64) - 1
    return (
        int(elem[0]) & mask,
        int(elem[1]) & mask,
        int(elem[2]) & mask,
        int(elem[3]) & mask,
    )


def _expect_field_elem_many(
    elems: Sequence[Sequence[int]], context: str
) -> Tuple[Tuple[int, int, int, int], ...]:
    return tuple(_expect_field_elem(elem, context) for elem in elems)


def bn254_add_cuda(a: Sequence[int], b: Sequence[int]) -> Optional[Tuple[int, int, int, int]]:
    """Add two BN254 field elements using the CUDA backend when available."""

    result = _crypto.bn254_add_cuda(_expect_field_elem(a, "bn254_add_cuda"), _expect_field_elem(b, "bn254_add_cuda"))
    if result is None:
        return None
    return _expect_field_elem(result, "bn254_add_cuda result")


def bn254_add_cuda_many(
    lhs: Sequence[Sequence[int]], rhs: Sequence[Sequence[int]]
) -> Optional[Tuple[Tuple[int, int, int, int], ...]]:
    """Add many BN254 field-element pairs using the CUDA backend when available."""

    if len(lhs) != len(rhs):
        raise ValueError("bn254_add_cuda_many expects matching batch lengths")
    result = _crypto.bn254_add_cuda_many(
        _expect_field_elem_many(lhs, "bn254_add_cuda_many lhs"),
        _expect_field_elem_many(rhs, "bn254_add_cuda_many rhs"),
    )
    if result is None:
        return None
    return tuple(_expect_field_elem(elem, "bn254_add_cuda_many result") for elem in result)


def bn254_sub_cuda(a: Sequence[int], b: Sequence[int]) -> Optional[Tuple[int, int, int, int]]:
    """Subtract two BN254 field elements using the CUDA backend when available."""

    result = _crypto.bn254_sub_cuda(_expect_field_elem(a, "bn254_sub_cuda"), _expect_field_elem(b, "bn254_sub_cuda"))
    if result is None:
        return None
    return _expect_field_elem(result, "bn254_sub_cuda result")


def bn254_sub_cuda_many(
    lhs: Sequence[Sequence[int]], rhs: Sequence[Sequence[int]]
) -> Optional[Tuple[Tuple[int, int, int, int], ...]]:
    """Subtract many BN254 field-element pairs using the CUDA backend when available."""

    if len(lhs) != len(rhs):
        raise ValueError("bn254_sub_cuda_many expects matching batch lengths")
    result = _crypto.bn254_sub_cuda_many(
        _expect_field_elem_many(lhs, "bn254_sub_cuda_many lhs"),
        _expect_field_elem_many(rhs, "bn254_sub_cuda_many rhs"),
    )
    if result is None:
        return None
    return tuple(_expect_field_elem(elem, "bn254_sub_cuda_many result") for elem in result)


def bn254_mul_cuda(a: Sequence[int], b: Sequence[int]) -> Optional[Tuple[int, int, int, int]]:
    """Multiply two BN254 field elements using the CUDA backend when available."""

    result = _crypto.bn254_mul_cuda(_expect_field_elem(a, "bn254_mul_cuda"), _expect_field_elem(b, "bn254_mul_cuda"))
    if result is None:
        return None
    return _expect_field_elem(result, "bn254_mul_cuda result")


def bn254_mul_cuda_many(
    lhs: Sequence[Sequence[int]], rhs: Sequence[Sequence[int]]
) -> Optional[Tuple[Tuple[int, int, int, int], ...]]:
    """Multiply many BN254 field-element pairs using the CUDA backend when available."""

    if len(lhs) != len(rhs):
        raise ValueError("bn254_mul_cuda_many expects matching batch lengths")
    result = _crypto.bn254_mul_cuda_many(
        _expect_field_elem_many(lhs, "bn254_mul_cuda_many lhs"),
        _expect_field_elem_many(rhs, "bn254_mul_cuda_many rhs"),
    )
    if result is None:
        return None
    return tuple(_expect_field_elem(elem, "bn254_mul_cuda_many result") for elem in result)
