import { blake3 } from '@noble/hashes/blake3.js';
import { normalizeAccountIdLiteral } from './accountLiteral.js';
const BASE58_ALPHABET = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz';
const BASE58_RE = /^[1-9A-HJ-NP-Za-km-z]+$/;
const ASSET_ALIAS_FORBIDDEN_SEGMENT_CHARS_RE = /[@#$:\s]/u;
const ASSET_DEFINITION_ADDRESS_VERSION = 1;
const ASSET_DEFINITION_ADDRESS_LEN = 21;
const ASSET_DEFINITION_CHECKSUM_OFFSET = 17;
const ASSET_DEFINITION_CHECKSUM_LEN = 4;
const ASSET_DEFINITION_UUID_VERSION_INDEX = 7;
const ASSET_DEFINITION_UUID_VARIANT_INDEX = 9;
const DATASPACE_SCOPE_RE = /^dataspace:\d+$/u;
const BASE58_VALUE_BY_CHAR = new Map([...BASE58_ALPHABET].map((char, index) => [char, index]));
function containsControlCharacters(value) {
    return /\p{Cc}/u.test(value);
}
function normalizeAliasSegment(segment) {
    const trimmed = segment.trim();
    if (!trimmed || trimmed !== segment)
        return null;
    if (ASSET_ALIAS_FORBIDDEN_SEGMENT_CHARS_RE.test(trimmed))
        return null;
    if (trimmed.includes('.') || containsControlCharacters(trimmed))
        return null;
    return trimmed.toLowerCase();
}
function decodeBase58(value) {
    if (!BASE58_RE.test(value))
        return null;
    const bytes = [0];
    for (const char of value) {
        const digit = BASE58_VALUE_BY_CHAR.get(char);
        if (digit === undefined)
            return null;
        let carry = digit;
        for (let index = 0; index < bytes.length; index += 1) {
            const next = bytes[index] * 58 + carry;
            bytes[index] = next & 0xff;
            carry = next >> 8;
        }
        while (carry > 0) {
            bytes.push(carry & 0xff);
            carry >>= 8;
        }
    }
    let leadingZeroCount = 0;
    while (leadingZeroCount < value.length && value[leadingZeroCount] === '1') {
        leadingZeroCount += 1;
    }
    const out = new Uint8Array(leadingZeroCount + bytes.length);
    for (let index = 0; index < leadingZeroCount; index += 1) {
        out[index] = 0;
    }
    for (let index = 0; index < bytes.length; index += 1) {
        out[out.length - 1 - index] = bytes[index];
    }
    return out;
}
function hasValidAssetDefinitionChecksum(decoded) {
    const expected = blake3(decoded.subarray(0, ASSET_DEFINITION_CHECKSUM_OFFSET))
        .subarray(0, ASSET_DEFINITION_CHECKSUM_LEN);
    for (let index = 0; index < ASSET_DEFINITION_CHECKSUM_LEN; index += 1) {
        if (decoded[ASSET_DEFINITION_CHECKSUM_OFFSET + index] !== expected[index])
            return false;
    }
    return true;
}
export function normalizeAssetDefinitionIdLiteral(value) {
    const trimmed = value.trim();
    if (!trimmed)
        return null;
    if (trimmed !== value)
        return null;
    if (trimmed.includes(':') || trimmed.includes('#') || trimmed.includes('@') || trimmed.includes('$'))
        return null;
    const decoded = decodeBase58(trimmed);
    if (!decoded || decoded.length !== ASSET_DEFINITION_ADDRESS_LEN)
        return null;
    if (decoded[0] !== ASSET_DEFINITION_ADDRESS_VERSION)
        return null;
    if ((decoded[ASSET_DEFINITION_UUID_VERSION_INDEX] >> 4) !== 0b0100)
        return null;
    if ((decoded[ASSET_DEFINITION_UUID_VARIANT_INDEX] & 0b1100_0000) !== 0b1000_0000)
        return null;
    if (!hasValidAssetDefinitionChecksum(decoded))
        return null;
    return trimmed;
}
export function parseAssetDefinitionAliasLiteral(value) {
    const trimmed = value.trim();
    if (!trimmed)
        return null;
    if (trimmed !== value)
        return null;
    if (containsControlCharacters(trimmed))
        return null;
    const [namePart, right] = trimmed.split('#');
    if (!namePart || !right)
        return null;
    if (trimmed.indexOf('#') !== trimmed.lastIndexOf('#'))
        return null;
    if (right.includes('@'))
        return null;
    const name = normalizeAliasSegment(namePart);
    if (!name)
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
            literal: `${name}#${domain}.${dataspace}`,
            name,
            domain,
            dataspace,
        };
    }
    const dataspace = normalizeAliasSegment(right);
    if (!dataspace)
        return null;
    return {
        literal: `${name}#${dataspace}`,
        name,
        domain: null,
        dataspace,
    };
}
export function normalizeAssetDefinitionAliasLiteral(value) {
    return parseAssetDefinitionAliasLiteral(value)?.literal ?? null;
}
export function normalizeAssetDefinitionSelectorLiteral(value) {
    return normalizeAssetDefinitionIdLiteral(value) ?? normalizeAssetDefinitionAliasLiteral(value);
}
export function parseAssetIdLiteral(value) {
    const trimmed = value.trim();
    if (!trimmed)
        return null;
    if (trimmed !== value)
        return null;
    const parts = trimmed.split('#');
    if (parts.length < 2 || parts.length > 3)
        return null;
    const definitionId = normalizeAssetDefinitionIdLiteral(parts[0] ?? '');
    const accountId = normalizeAccountIdLiteral(parts[1] ?? '');
    if (!definitionId || !accountId)
        return null;
    const scope = parts[2] ?? null;
    if (scope && !DATASPACE_SCOPE_RE.test(scope))
        return null;
    return {
        literal: scope ? `${definitionId}#${accountId}#${scope}` : `${definitionId}#${accountId}`,
        definitionId,
        accountId,
        scope,
    };
}
export function normalizeAssetIdLiteral(value) {
    return parseAssetIdLiteral(value)?.literal ?? null;
}
export function extractAssetDefinitionIdFromAssetIdLiteral(value) {
    return parseAssetIdLiteral(value)?.definitionId ?? null;
}
