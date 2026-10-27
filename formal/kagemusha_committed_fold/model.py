"""Finite scalar-log controls for the committed fold, not native/RP57 execution.

Forward vector folding and inverse closed-form projections are separate algorithms.
Only the deterministic toy digest/transcript and checked experiment data are shared.
The complete tape is selected before the first message; zero messages stop once.
"""

from dataclasses import dataclass
from itertools import product
from math import prod


@dataclass(frozen=True)
class Shape:
    """Fixed bounded cyclic-group experiment, with nonzero W and U logs."""

    p: int
    k: int
    generators: tuple
    w: int
    auxiliary: int
    context: tuple = (1, 2, 3, 2)


@dataclass(frozen=True)
class Source:
    """One deciding claim with its actual (unpadded) nonzero challenge width."""

    challenges: tuple
    claim: int


@dataclass(frozen=True)
class Trace:
    """Private scalar witness plus the stopped point-path projection.

    A terminal zero records the failed point attempt, not an encoded message.
    The coefficient and gamma are private and excluded from public_key().
    """

    points: tuple
    challenges: tuple
    stop: int | None
    coefficient: int | None = None
    generator: int | None = None
    gamma: int | None = None

    def public_key(self):
        """Logical path/error projection; successful G is determined by u."""
        return self.points, self.challenges, self.stop, self.generator


def canonical(value, p):
    """Refuse Python booleans and noncanonical field words."""
    if type(value) is not int or not 0 <= value < p:
        raise ValueError('canonical field word required')


def source_coefficients(challenges, p):
    """Bit-product definition, independent of inverse's doubling expansion."""
    width = len(challenges)
    return tuple(prod(u for j, u in enumerate(challenges)
                      if index & (1 << (width - 1 - j))) % p
                 for index in range(1 << width))


def source(shape, challenges):
    """Make a deciding toy claim; this grants no native admission authority."""
    coefficients = source_coefficients(challenges, shape.p)
    claim = sum(a * g for a, g in zip(coefficients, shape.generators)) % shape.p
    return Source(tuple(challenges), claim)


def validate(shape, inputs):
    """Bound every exhaustive domain and require fixed admissible deciding inputs."""
    if type(shape.p) is not int or shape.p not in (5, 7):
        raise ValueError('only the prime fields 5 and 7 are supported')
    if type(shape.k) is not int or shape.k not in (1, 2):
        raise ValueError('one or two control rounds required')
    if len(shape.generators) != 1 << shape.k or len(shape.context) != 4:
        raise ValueError('complete generator/context extent required')
    for word in (*shape.generators, shape.w, shape.auxiliary, *shape.context):
        canonical(word, shape.p)
    if 0 in (*shape.generators, shape.w, shape.auxiliary):
        raise ValueError('finite parameter points required')
    if not 1 <= len(inputs) <= 4 or shape.context[3] != len(inputs):
        raise ValueError('fixed bounded slot count required')
    if not any(len(item.challenges) == shape.k for item in inputs):
        raise ValueError('explicit full-width source required')
    for item in inputs:
        if not 1 <= len(item.challenges) <= shape.k:
            raise ValueError('source width must be one through the control depth')
        canonical(item.claim, shape.p)
        for word in item.challenges:
            canonical(word, shape.p)
            if word == 0:
                raise ValueError('actual source challenges are nonzero')
        if source(shape, item.challenges) != item:
            raise ValueError('source claim does not decide')


def toy_hash(words, modulus):
    """An explicitly public deterministic function; no random-oracle claim."""
    state = 1
    for index, word in enumerate(words):
        state = (3 * state + word + index + 1) % modulus
        state = (state * state + 2 * state + index) % modulus
    return state


def digest_limbs(shape, inputs, masks, *, omit=None):
    """Toy framed digest with every ordered mask, split into two radix-p limbs.

    This models dependence and opening algebra, not Pasta's 128-bit encoding or
    RP57. omit is an explicit mutation control, never a source-compatible mode.
    """
    words = [11, *shape.context]
    for item in inputs:
        words.extend((item.claim, len(item.challenges), *item.challenges))
    words.extend(value for index, value in enumerate(masks) if index != omit)
    value = toy_hash((len(words), *words), shape.p ** 2)
    return value % shape.p, value // shape.p


def digest_offset(shape, inputs, masks, *, omit=None):
    """The logarithm of g0*h_low + g1*h_high in this toy group."""
    low, high = digest_limbs(shape, inputs, masks, omit=omit)
    return (shape.generators[0] * low + shape.generators[1] * high) % shape.p


def initial_challenges(shape, d):
    """Fixed public context then D; alpha/z/zeta may be zero."""
    return tuple(toy_hash((19, *shape.context, d, index), shape.p) for index in range(3))


def round_challenge(shape, points):
    """A deterministic path function followed by native's zero-to-one role map."""
    raw = toy_hash((23, *shape.context, *points), shape.p)
    return raw if raw else 1


def forward(shape, inputs, tape, *, omit=None):
    """Sample-order algorithm: commit a complete tape, then fold mutable vectors."""
    validate(shape, inputs)
    p, k = shape.p, shape.k
    if len(tape) != 1 + 2 * k:
        raise ValueError('complete preselected tape required')
    for word in tape:
        canonical(word, p)
    rho, *masks = tape
    d = (digest_offset(shape, inputs, masks, omit=omit) + shape.w * rho) % p
    points, challenges = [d], []
    if d == 0:
        return Trace(tuple(points), (), 0)
    alpha, z, zeta = initial_challenges(shape, d)
    a = [0] * (1 << k)
    weight = 1
    for item in inputs:
        for index, coefficient in enumerate(source_coefficients(item.challenges, p)):
            a[index] = (a[index] + weight * coefficient) % p
        weight = weight * alpha % p
    b = [pow(z, index, p) for index in range(len(a))]
    a[0] = (a[0] - sum(x * y for x, y in zip(a, b))) % p
    g, gamma = list(shape.generators), 0
    for j in range(k):
        half = len(a) // 2
        # Native computes both points before ordered finite-point writes.
        left = (sum(a[i + half] * (g[i] + shape.auxiliary * zeta * b[i])
                    for i in range(half)) + shape.w * masks[2 * j]) % p
        right = (sum(a[i] * (g[i + half] + shape.auxiliary * zeta * b[i + half])
                     for i in range(half)) + shape.w * masks[2 * j + 1]) % p
        for point in (left, right):
            points.append(point)
            if point == 0:
                return Trace(tuple(points), tuple(challenges), len(points) - 1)
        u = round_challenge(shape, points)
        challenges.append(u)
        inverse_u = pow(u, -1, p)
        gamma = (gamma + masks[2 * j] * inverse_u + masks[2 * j + 1] * u) % p
        a = [(a[i] + inverse_u * a[i + half]) % p for i in range(half)]
        b = [(b[i] + u * b[i + half]) % p for i in range(half)]
        g = [(g[i] + u * g[i + half]) % p for i in range(half)]
    return Trace(tuple(points), tuple(challenges), None, a[0], g[0], gamma)


def path_shape(shape, points):
    """Return the first failed position, or accept an exact complete finite path."""
    if not 1 <= len(points) <= 1 + 2 * shape.k:
        raise ValueError('bounded point path required')
    for point in points:
        canonical(point, shape.p)
    zeros = [index for index, point in enumerate(points) if point == 0]
    if zeros:
        if zeros != [len(points) - 1]:
            raise ValueError('stop at the first identity without continuation')
        return zeros[0]
    if len(points) != 1 + 2 * shape.k:
        raise ValueError('a nonstopped path must be complete')
    return None


def inverse(shape, inputs, points, completion=(), *, omit=None, early_rho=False):
    """Invert a stopped path using closed-form projections, never forward().

    Free completion coordinates are precisely the masks after the failed point.
    All masks, including those invisible after refusal, precede the rho solve.
    early_rho deliberately violates that order for a negative control.
    """
    validate(shape, inputs)
    p, k = shape.p, shape.k
    stop = path_shape(shape, points)
    observed_masks = len(points) - 1
    if len(completion) != 2 * k - observed_masks:
        raise ValueError('exact unused-mask completion required')
    for word in completion:
        canonical(word, p)
    masks = []
    d = points[0]
    if stop != 0:
        alpha, z, zeta = initial_challenges(shape, d)
        # Independent polynomial expansion; no forward coefficient helper.
        original = [0] * (1 << k)
        evaluation = 0
        for input_index, item in enumerate(inputs):
            expanded = [1]
            value, power = 1, z
            for u in reversed(item.challenges):
                expanded += [x * u % p for x in expanded]
                value = value * (1 + u * power) % p
                power = power * power % p
            scale = pow(alpha, input_index, p)
            evaluation = (evaluation + scale * value) % p
            for index, coefficient in enumerate(expanded):
                original[index] = (original[index] + scale * coefficient) % p
        original[0] = (original[0] - evaluation) % p
        prior = []
        for j in range(k):
            length = (1 << k) >> j
            half = length // 2
            # Project original arrays directly through all prior challenges.
            weights = [prod(prior[t] for t in range(j)
                            if high & (1 << (j - 1 - t))) % p
                       for high in range(1 << j)]
            a, b, g = [], [], []
            for index in range(length):
                a.append(sum(original[index + high * length] * pow(weight, -1, p)
                             for high, weight in enumerate(weights)) % p)
                b.append(sum(pow(z, index + high * length, p) * weight
                             for high, weight in enumerate(weights)) % p)
                g.append(sum(shape.generators[index + high * length] * weight
                             for high, weight in enumerate(weights)) % p)
            offsets = (
                sum(a[i + half] * g[i] for i in range(half))
                + shape.auxiliary * zeta * sum(a[i + half] * b[i] for i in range(half)),
                sum(a[i] * g[i + half] for i in range(half))
                + shape.auxiliary * zeta * sum(a[i] * b[i + half] for i in range(half)),
            )
            for side, offset in enumerate(offsets):
                position = 1 + 2 * j + side
                if position >= len(points):
                    break
                masks.append((points[position] - offset) * pow(shape.w, -1, p) % p)
            if len(masks) != 2 * (j + 1) or stop == 2 * (j + 1):
                break
            prior.append(round_challenge(shape, points[:1 + 2 * (j + 1)]))
    early_offset = digest_offset(shape, inputs, masks + [0] * len(completion), omit=omit)
    masks.extend(completion)
    offset = early_offset if early_rho else digest_offset(shape, inputs, masks, omit=omit)
    rho = (d - offset) * pow(shape.w, -1, p) % p
    return (rho, *masks)


def inverse_domain(shape):
    """Every finite path and every unused-mask completion, each exactly once."""
    if type(shape.p) is not int or shape.p not in (5, 7) or type(shape.k) is not int or shape.k not in (1, 2):
        raise ValueError('only bounded prime-field control shapes can be enumerated')
    p, width = shape.p, 1 + 2 * shape.k
    for stop in range(width):
        for prefix in product(range(1, p), repeat=stop):
            for completion in product(range(p), repeat=width - 1 - stop):
                yield (*prefix, 0), completion
    for points in product(range(1, p), repeat=width):
        yield points, ()


def equation_residual(shape, inputs, trace, masks, *, gamma_sign=-1):
    """Independently check the complete pure-generator equation on success.

    gamma_sign=0 or +1 are explicit verifier mutations, not alternate modes.
    """
    if trace.stop is not None:
        raise ValueError('a stopped path has no final equation')
    p = shape.p
    alpha, z, zeta = initial_challenges(shape, trace.points[0])
    commitment, evaluation = 0, 0
    for item in reversed(inputs):
        commitment = (commitment * alpha + item.claim) % p
        value, power = 1, z
        for u in reversed(item.challenges):
            value = value * (1 + u * power) % p
            power = power * power % p
        evaluation = (evaluation * alpha + value) % p
    lhs, gamma = commitment - evaluation * shape.generators[0], 0
    for j, u in enumerate(trace.challenges):
        inv = pow(u, -1, p)
        lhs += inv * trace.points[1 + 2 * j] + u * trace.points[2 + 2 * j]
        gamma += inv * masks[2 * j] + u * masks[2 * j + 1]
    b, power = 1, z
    for u in reversed(trace.challenges):
        b = b * (1 + u * power) % p
        power = power * power % p
    return (lhs + gamma_sign * shape.w * gamma
            - trace.coefficient * (trace.generator + shape.auxiliary * zeta * b)) % p


def decided_generator(shape, challenges):
    """Full bit-product decision, independent of the forward folded vector."""
    return sum(g * coefficient for g, coefficient in
               zip(shape.generators, source_coefficients(challenges, shape.p))) % shape.p
