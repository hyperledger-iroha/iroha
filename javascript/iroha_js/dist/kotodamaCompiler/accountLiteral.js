const BASE58_ALPHABET = [
    '1',
    '2',
    '3',
    '4',
    '5',
    '6',
    '7',
    '8',
    '9',
    'A',
    'B',
    'C',
    'D',
    'E',
    'F',
    'G',
    'H',
    'J',
    'K',
    'L',
    'M',
    'N',
    'P',
    'Q',
    'R',
    'S',
    'T',
    'U',
    'V',
    'W',
    'X',
    'Y',
    'Z',
    'a',
    'b',
    'c',
    'd',
    'e',
    'f',
    'g',
    'h',
    'i',
    'j',
    'k',
    'm',
    'n',
    'o',
    'p',
    'q',
    'r',
    's',
    't',
    'u',
    'v',
    'w',
    'x',
    'y',
    'z',
];
const IROHA_POEM_KANA_HALFWIDTH = [
    'ｲ',
    'ﾛ',
    'ﾊ',
    'ﾆ',
    'ﾎ',
    'ﾍ',
    'ﾄ',
    'ﾁ',
    'ﾘ',
    'ﾇ',
    'ﾙ',
    'ｦ',
    'ﾜ',
    'ｶ',
    'ﾖ',
    'ﾀ',
    'ﾚ',
    'ｿ',
    'ﾂ',
    'ﾈ',
    'ﾅ',
    'ﾗ',
    'ﾑ',
    'ｳ',
    'ヰ',
    'ﾉ',
    'ｵ',
    'ｸ',
    'ﾔ',
    'ﾏ',
    'ｹ',
    'ﾌ',
    'ｺ',
    'ｴ',
    'ﾃ',
    'ｱ',
    'ｻ',
    'ｷ',
    'ﾕ',
    'ﾒ',
    'ﾐ',
    'ｼ',
    'ヱ',
    'ﾋ',
    'ﾓ',
    'ｾ',
    'ｽ',
];
const NORITO_LITERAL_RE = /^norito:[0-9a-f]+$/i;
const ACCOUNT_ALIAS_FORBIDDEN_SEGMENT_CHARS_RE = /[@#$:\s]/u;
const I105_BASE = 105;
const I105_CHECKSUM_LEN = 6;
const I105_SENTINEL_SORA = 'sora';
const I105_SENTINEL_TEST = 'test';
const I105_SENTINEL_DEV = 'dev';
const I105_SENTINEL_FALLBACK_PREFIX = 'n';
const DEFAULT_I105_CHAIN_DISCRIMINANT = 0x02f1;
const TEST_I105_CHAIN_DISCRIMINANT = 0x0171;
const DEV_I105_CHAIN_DISCRIMINANT = 0x0000;
const BECH32M_CONST = 0x2bc8_30a3;
const ACCOUNT_ADDRESS_HEADER_SINGLE_KEY_V1 = 0x02;
const ACCOUNT_CONTROLLER_SINGLE_KEY_TAG = 0x00;
const CURVE_ID_ED25519 = 1;
const CURVE_ID_MLDSA = 2;
const CURVE_ID_SECP256K1 = 4;
const CURVE_ID_SM2 = 15;
const MULTIHASH_PUBLIC_DIGEST_ED25519 = 0xed;
const MULTIHASH_PUBLIC_DIGEST_SECP256K1 = 0xe7;
const MULTIHASH_PUBLIC_DIGEST_MLDSA = 0xee;
const MULTIHASH_PUBLIC_DIGEST_SM2 = 0x1306;
const MODERN_I105_ALPHABET = [...BASE58_ALPHABET, ...IROHA_POEM_KANA_HALFWIDTH];
const MODERN_I105_VALUE_BY_CHAR = new Map(MODERN_I105_ALPHABET.map((char, index) => [char, index]));
const ALGORITHM_PREFIX_TO_DIGEST = new Map([
    ['ed25519', MULTIHASH_PUBLIC_DIGEST_ED25519],
    ['secp256k1', MULTIHASH_PUBLIC_DIGEST_SECP256K1],
    ['ml-dsa', MULTIHASH_PUBLIC_DIGEST_MLDSA],
    ['mldsa', MULTIHASH_PUBLIC_DIGEST_MLDSA],
    ['sm2', MULTIHASH_PUBLIC_DIGEST_SM2],
]);
function escapeRegExp(value) {
    return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}
const MODERN_I105_SYMBOL_PATTERN = [...BASE58_ALPHABET, ...IROHA_POEM_KANA_HALFWIDTH].map(escapeRegExp).join('|');
const MODERN_I105_ACCOUNT_ID_RE = new RegExp(`^(?:sora|test|dev|n[0-9]{1,5})(?:${MODERN_I105_SYMBOL_PATTERN})+$`, 'u');
const ACCOUNT_ID_SENTINEL_RE = /^(sora|test|dev|n[0-9]{1,5})(.*)$/u;
function normalizeAliasSegment(segment) {
    const trimmed = segment.trim();
    if (!trimmed || trimmed !== segment)
        return null;
    if (ACCOUNT_ALIAS_FORBIDDEN_SEGMENT_CHARS_RE.test(trimmed))
        return null;
    if ([...trimmed].some(char => char === '.' || char === ':'))
        return null;
    if (containsControlCharacters(trimmed))
        return null;
    return trimmed.toLowerCase();
}
function containsControlCharacters(value) {
    return /\p{Cc}/u.test(value);
}
function decodeHexBytes(value) {
    if (value.length === 0 || value.length % 2 !== 0 || !/^[0-9a-f]+$/iu.test(value))
        return null;
    const bytes = new Uint8Array(value.length / 2);
    for (let index = 0; index < bytes.length; index += 1) {
        const next = Number.parseInt(value.slice(index * 2, index * 2 + 2), 16);
        if (Number.isNaN(next))
            return null;
        bytes[index] = next;
    }
    return bytes;
}
function readVarUint(bytes, offset) {
    let value = 0;
    let shift = 0;
    let index = offset;
    while (index < bytes.length) {
        const byte = bytes[index];
        value |= (byte & 0x7f) << shift;
        index += 1;
        if ((byte & 0x80) === 0) {
            return { value, nextOffset: index };
        }
        shift += 7;
        if (shift > 28)
            return null;
    }
    return null;
}
function decodeCanonicalPublicKeyMultihash(value) {
    const trimmed = value.trim();
    if (!trimmed || trimmed !== value)
        return null;
    const [prefix, hexLiteral] = trimmed.includes(':')
        ? trimmed.split(':', 2)
        : [null, trimmed];
    const expectedDigest = prefix === null ? null : ALGORITHM_PREFIX_TO_DIGEST.get(prefix.toLowerCase()) ?? null;
    if (prefix !== null && expectedDigest === null)
        return null;
    const bytes = decodeHexBytes(hexLiteral ?? '');
    if (!bytes)
        return null;
    const digest = readVarUint(bytes, 0);
    if (!digest)
        return null;
    const length = readVarUint(bytes, digest.nextOffset);
    if (!length)
        return null;
    if (length.nextOffset + length.value !== bytes.length)
        return null;
    if (expectedDigest !== null && digest.value !== expectedDigest)
        return null;
    return {
        digestFunction: digest.value,
        payload: bytes.slice(length.nextOffset),
    };
}
function curveIdForPublicKeyDigest(digestFunction) {
    switch (digestFunction) {
        case MULTIHASH_PUBLIC_DIGEST_ED25519:
            return CURVE_ID_ED25519;
        case MULTIHASH_PUBLIC_DIGEST_SECP256K1:
            return CURVE_ID_SECP256K1;
        case MULTIHASH_PUBLIC_DIGEST_MLDSA:
            return CURVE_ID_MLDSA;
        case MULTIHASH_PUBLIC_DIGEST_SM2:
            return CURVE_ID_SM2;
        default:
            return null;
    }
}
function publicKeyDigestForCurveId(curveId) {
    switch (curveId) {
        case CURVE_ID_ED25519:
            return MULTIHASH_PUBLIC_DIGEST_ED25519;
        case CURVE_ID_SECP256K1:
            return MULTIHASH_PUBLIC_DIGEST_SECP256K1;
        case CURVE_ID_MLDSA:
            return MULTIHASH_PUBLIC_DIGEST_MLDSA;
        case CURVE_ID_SM2:
            return MULTIHASH_PUBLIC_DIGEST_SM2;
        default:
            return null;
    }
}
function encodeBaseNDigits(bytes, base) {
    if (base < 2)
        return null;
    if (bytes.length === 0)
        return [0];
    const leadingZeros = bytes.findIndex((byte) => byte !== 0);
    const zeroCount = leadingZeros === -1 ? bytes.length : leadingZeros;
    const value = [...bytes];
    const digits = [];
    let start = zeroCount;
    while (start < value.length) {
        let remainder = 0;
        for (let index = start; index < value.length; index += 1) {
            const accumulator = (remainder << 8) | value[index];
            value[index] = Math.floor(accumulator / base);
            remainder = accumulator % base;
        }
        digits.push(remainder);
        while (start < value.length && value[start] === 0) {
            start += 1;
        }
    }
    while (digits.length < zeroCount) {
        digits.push(0);
    }
    if (digits.length === 0)
        digits.push(0);
    digits.reverse();
    return digits;
}
function decodeModernI105Digits(value) {
    const digits = [];
    for (const char of value) {
        const digit = MODERN_I105_VALUE_BY_CHAR.get(char);
        if (digit === undefined)
            return null;
        digits.push(digit);
    }
    return digits;
}
function decodeBaseNDigits(digits, base) {
    if (base < 2 || digits.length === 0)
        return null;
    const bytes = [0];
    for (const digit of digits) {
        if (!Number.isInteger(digit) || digit < 0 || digit >= base)
            return null;
        let carry = digit;
        for (let index = 0; index < bytes.length; index += 1) {
            const next = bytes[index] * base + carry;
            bytes[index] = next & 0xff;
            carry = next >> 8;
        }
        while (carry > 0) {
            bytes.push(carry & 0xff);
            carry >>= 8;
        }
    }
    let leadingZeroCount = 0;
    while (leadingZeroCount < digits.length && digits[leadingZeroCount] === 0) {
        leadingZeroCount += 1;
    }
    const out = new Uint8Array(leadingZeroCount + bytes.length);
    for (let index = 0; index < bytes.length; index += 1) {
        out[out.length - 1 - index] = bytes[index];
    }
    return out;
}
function encodeVarUint(value) {
    if (!Number.isInteger(value) || value < 0)
        return null;
    const bytes = [];
    let remaining = value;
    do {
        let byte = remaining & 0x7f;
        remaining >>>= 7;
        if (remaining > 0)
            byte |= 0x80;
        bytes.push(byte);
    } while (remaining > 0);
    return Uint8Array.from(bytes);
}
function encodeHexUpper(bytes) {
    return [...bytes].map((byte) => byte.toString(16).padStart(2, '0').toUpperCase()).join('');
}
function convertToBase32Digits(bytes) {
    let accumulator = 0;
    let bits = 0;
    const out = [];
    for (const byte of bytes) {
        accumulator = (accumulator << 8) | byte;
        bits += 8;
        while (bits >= 5) {
            bits -= 5;
            out.push((accumulator >> bits) & 0x1f);
        }
    }
    if (bits > 0) {
        out.push((accumulator << (5 - bits)) & 0x1f);
    }
    return out;
}
function expandHrp(value) {
    const out = [];
    for (const char of value) {
        const code = char.charCodeAt(0);
        out.push(code >> 5);
    }
    out.push(0);
    for (const char of value) {
        out.push(char.charCodeAt(0) & 0x1f);
    }
    return out;
}
function bech32Polymod(values) {
    const generators = [
        0x3b6a_57b2,
        0x2650_8e6d,
        0x1ea1_19fa,
        0x3d42_33dd,
        0x2a14_62b3,
    ];
    let checksum = 1;
    for (const value of values) {
        const top = checksum >>> 25;
        checksum = ((checksum & 0x1ff_ffff) << 5) ^ value;
        for (let index = 0; index < generators.length; index += 1) {
            if (((top >>> index) & 1) === 1) {
                checksum ^= generators[index];
            }
        }
    }
    return checksum >>> 0;
}
function i105ChecksumDigits(canonical) {
    const values = expandHrp('snx');
    values.push(...convertToBase32Digits(canonical));
    values.push(...new Array(I105_CHECKSUM_LEN).fill(0));
    const polymod = bech32Polymod(values) ^ BECH32M_CONST;
    const result = [];
    for (let index = 0; index < I105_CHECKSUM_LEN; index += 1) {
        const shift = 5 * (I105_CHECKSUM_LEN - 1 - index);
        result.push((polymod >>> shift) & 0x1f);
    }
    return result;
}
function i105SentinelForDiscriminant(discriminant) {
    switch (discriminant) {
        case DEFAULT_I105_CHAIN_DISCRIMINANT:
            return I105_SENTINEL_SORA;
        case TEST_I105_CHAIN_DISCRIMINANT:
            return I105_SENTINEL_TEST;
        case DEV_I105_CHAIN_DISCRIMINANT:
            return I105_SENTINEL_DEV;
        default:
            return `${I105_SENTINEL_FALLBACK_PREFIX}${discriminant}`;
    }
}
function buildSingleKeyAccountCanonicalBytes(curveId, payload) {
    if (payload.length > 0xff)
        return null;
    return Uint8Array.from([
        ACCOUNT_ADDRESS_HEADER_SINGLE_KEY_V1,
        ACCOUNT_CONTROLLER_SINGLE_KEY_TAG,
        curveId,
        payload.length,
        ...payload,
    ]);
}
export function renderCanonicalAccountIdLiteralFromPublicKeyLiteral(value, discriminant = DEFAULT_I105_CHAIN_DISCRIMINANT) {
    const multihash = decodeCanonicalPublicKeyMultihash(value);
    if (!multihash)
        return null;
    const curveId = curveIdForPublicKeyDigest(multihash.digestFunction);
    if (curveId === null)
        return null;
    const canonical = buildSingleKeyAccountCanonicalBytes(curveId, multihash.payload);
    if (!canonical)
        return null;
    const digits = encodeBaseNDigits(canonical, I105_BASE);
    if (!digits)
        return null;
    let output = i105SentinelForDiscriminant(discriminant);
    for (const digit of digits) {
        output += MODERN_I105_ALPHABET[digit];
    }
    for (const digit of i105ChecksumDigits(canonical)) {
        output += MODERN_I105_ALPHABET[digit];
    }
    return normalizeAccountIdLiteral(output);
}
export function renderCanonicalPublicKeyLiteralFromAccountIdLiteral(value) {
    const accountId = normalizeAccountIdLiteral(value);
    if (!accountId || !MODERN_I105_ACCOUNT_ID_RE.test(accountId))
        return null;
    const match = ACCOUNT_ID_SENTINEL_RE.exec(accountId);
    const encodedDigits = match?.[2] ?? '';
    const digits = decodeModernI105Digits(encodedDigits);
    if (!digits || digits.length <= I105_CHECKSUM_LEN)
        return null;
    const canonicalDigits = digits.slice(0, -I105_CHECKSUM_LEN);
    const checksumDigits = digits.slice(-I105_CHECKSUM_LEN);
    const canonical = decodeBaseNDigits(canonicalDigits, I105_BASE);
    if (!canonical)
        return null;
    const expectedChecksum = i105ChecksumDigits(canonical);
    if (expectedChecksum.length !== checksumDigits.length)
        return null;
    if (expectedChecksum.some((digit, index) => digit !== checksumDigits[index]))
        return null;
    if (canonical.length < 4 || canonical[0] !== ACCOUNT_ADDRESS_HEADER_SINGLE_KEY_V1 || canonical[1] !== ACCOUNT_CONTROLLER_SINGLE_KEY_TAG) {
        return null;
    }
    const curveId = canonical[2];
    const payloadLength = canonical[3];
    if (canonical.length !== 4 + payloadLength)
        return null;
    const digestFunction = publicKeyDigestForCurveId(curveId);
    if (digestFunction === null)
        return null;
    const payload = canonical.slice(4);
    const digestBytes = encodeVarUint(digestFunction);
    const lengthBytes = encodeVarUint(payload.length);
    if (!digestBytes || !lengthBytes)
        return null;
    return encodeHexUpper(Uint8Array.from([...digestBytes, ...lengthBytes, ...payload]));
}
export function normalizeAccountIdLiteral(value) {
    const trimmed = value.trim();
    if (!trimmed)
        return null;
    if (NORITO_LITERAL_RE.test(trimmed))
        return null;
    if (trimmed.includes('@') || trimmed.includes('#') || trimmed.includes('$') || trimmed.includes(':'))
        return null;
    return MODERN_I105_ACCOUNT_ID_RE.test(trimmed) ? trimmed : null;
}
export function parseAccountAliasLiteral(value) {
    const trimmed = value.trim();
    if (!trimmed)
        return null;
    if (trimmed !== value)
        return null;
    if (containsControlCharacters(trimmed))
        return null;
    const [labelPart, right] = trimmed.split('@');
    if (!labelPart || !right)
        return null;
    if (trimmed.indexOf('@') !== trimmed.lastIndexOf('@'))
        return null;
    const label = normalizeAliasSegment(labelPart);
    if (!label)
        return null;
    const dotCount = [...right].filter(char => char === '.').length;
    if (dotCount > 1)
        return null;
    if (dotCount === 1) {
        const [domainPart, dataspacePart] = right.split('.');
        const domain = normalizeAliasSegment(domainPart ?? '');
        const dataspace = normalizeAliasSegment(dataspacePart ?? '');
        if (!domain || !dataspace)
            return null;
        return {
            literal: `${label}@${domain}.${dataspace}`,
            label,
            domain,
            dataspace,
        };
    }
    const dataspace = normalizeAliasSegment(right);
    if (!dataspace)
        return null;
    return {
        literal: `${label}@${dataspace}`,
        label,
        domain: null,
        dataspace,
    };
}
export function normalizeAccountAliasLiteral(value) {
    return parseAccountAliasLiteral(value)?.literal ?? null;
}
export function normalizeAccountSelectorLiteral(value) {
    return normalizeAccountIdLiteral(value) ?? normalizeAccountAliasLiteral(value);
}
export function normalizeDisplayedAccountSelectorLiteral(value, _toriiBaseUrl) {
    const accountId = normalizeAccountIdLiteral(value);
    if (accountId)
        return accountId;
    return normalizeAccountAliasLiteral(value);
}
export function normalizeToriiAccountSelectorLiteral(value, _toriiBaseUrl) {
    const accountId = normalizeAccountIdLiteral(value);
    if (accountId)
        return accountId;
    return normalizeAccountAliasLiteral(value);
}
export function isEncodedAccountLiteral(value) {
    return normalizeAccountIdLiteral(value) !== null;
}
export function isAccountSelectorLiteral(value) {
    return normalizeAccountSelectorLiteral(value) !== null;
}
export function normalizeEncodedAccountLiteral(value) {
    return normalizeAccountIdLiteral(value);
}
export function normalizeLooseAccountLiteral(value) {
    return normalizeAccountSelectorLiteral(value);
}
export function isEncodedAssetLiteral(value) {
    return NORITO_LITERAL_RE.test(value.trim());
}
