"""Small integer helpers matching ``src/math.rs`` and ``div_ceil``.

``log2`` reproduces the crate's ``Math::log_2`` exactly:

* for a power of two, the exact base-2 logarithm;
* otherwise, ``ceil(log2(n))``.

Both cases equal ``(n - 1).bit_length()`` for ``n >= 1``.
"""


def log2(n: int) -> int:
  """``Math::log_2`` for ``usize`` (asserts ``n != 0``)."""
  if n == 0:
    raise ValueError("log2(0) is undefined")
  return (n - 1).bit_length()


def ilog2(n: int) -> int:
  """``usize::ilog2`` -- floor of the base-2 logarithm (asserts ``n != 0``)."""
  if n <= 0:
    raise ValueError("ilog2 requires n >= 1")
  return n.bit_length() - 1


def next_power_of_two(n: int) -> int:
  """``usize::next_power_of_two`` -- smallest power of two ``>= n`` (0 -> 1)."""
  if n <= 1:
    return 1
  return 1 << (n - 1).bit_length()


def div_ceil(a: int, b: int) -> int:
  """Ceiling division ``ceil(a / b)`` for non-negative ``a`` and positive ``b``."""
  return (a + b - 1) // b
