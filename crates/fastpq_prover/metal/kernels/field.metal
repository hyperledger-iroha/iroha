#ifndef FASTPQ_FIELD_METAL
#define FASTPQ_FIELD_METAL

constant ulong FIELD_MODULUS = 0xffffffff00000001UL;
constant ulong FIELD_GENERATOR = 7UL;

inline ulong reduce_goldilocks(ulong lo, ulong hi) {
    uint lo_lo = (uint)(lo & 0xffffffffUL);
    uint lo_hi = (uint)(lo >> 32);
    uint hi_lo = (uint)(hi & 0xffffffffUL);
    uint hi_hi = (uint)(hi >> 32);

    ulong acc = ((ulong)(lo_hi + hi_lo) << 32) + (ulong)(lo_lo);

    ulong sub = (ulong)(hi_lo);
    if (acc < sub) {
        acc += FIELD_MODULUS;
    }
    acc -= sub;

    sub = (ulong)(hi_hi);
    if (acc < sub) {
        acc += FIELD_MODULUS;
    }
    acc -= sub;

    if (acc >= FIELD_MODULUS) {
        acc -= FIELD_MODULUS;
    }
    if (acc >= FIELD_MODULUS) {
        acc -= FIELD_MODULUS;
    }
    return acc;
}

inline ulong reduce_goldilocks(ulong2 wide) {
    return reduce_goldilocks(wide.x, wide.y);
}

inline ulong add_mod(ulong a, ulong b) {
    ulong sum = a + b;
    if (sum < a) {
        return reduce_goldilocks(sum, 1UL);
    }
    if (sum >= FIELD_MODULUS) {
        sum -= FIELD_MODULUS;
    }
    return sum;
}

inline ulong sub_mod(ulong a, ulong b) {
    if (a >= b) {
        return a - b;
    }
    return FIELD_MODULUS - (b - a);
}

inline ulong mul_mod(ulong a, ulong b) {
    ulong lo = a * b;
    ulong hi = mulhi(a, b);
    return reduce_goldilocks(lo, hi);
}

inline ulong pow5(ulong x) {
    ulong x2 = mul_mod(x, x);
    ulong x4 = mul_mod(x2, x2);
    return mul_mod(x4, x);
}

inline ulong pow_mod(ulong base, ulong exponent) {
    ulong result = 1UL;
    ulong power = base;
    ulong exp = exponent;
    while (exp != 0UL) {
        if (exp & 1UL) {
            result = mul_mod(result, power);
        }
        power = mul_mod(power, power);
        exp >>= 1UL;
    }
    return result;
}

inline ulong multiplicative_inverse(ulong value) {
    return pow_mod(value, FIELD_MODULUS - 2UL);
}

inline ulong pow_mod_small(ulong base, uint exp) {
    ulong result = 1UL;
    for (uint i = 0U; i < exp; ++i) {
        result = mul_mod(result, base);
    }
    return result;
}

#endif // FASTPQ_FIELD_METAL
