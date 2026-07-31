import { CborDecoderBase } from './CborDecoderBase';
import { JsonPackValue } from '../JsonPackValue';
import type { Path } from '@jsonjoy.com/json-pointer';
import type { IReader, IReaderResettable } from '@jsonjoy.com/buffers';
export declare class CborDecoder<R extends IReader & IReaderResettable = IReader & IReaderResettable> extends CborDecoderBase<R> {
    readAsMap(): Map<unknown, unknown>;
    readMap(minor: number): Map<unknown, unknown>;
    readMapRaw(length: number): Map<unknown, unknown>;
    readMapIndef(): Map<unknown, unknown>;
    skipN(n: number): void;
    skipAny(): void;
    skipAnyRaw(octet: number): void;
    skipMinorLen(minor: number): number;
    skipUNint(minor: number): void;
    skipBin(minor: number): void;
    skipBinChunk(): void;
    skipStr(minor: number): void;
    skipStrChunk(): void;
    skipArr(minor: number): void;
    skipObj(minor: number): void;
    skipTag(minor: number): void;
    skipTkn(minor: number): void;
    /**
     * Throws if at given offset in a buffer there is an invalid CBOR value, or
     * if the value does not span the exact length specified in `size`. I.e.
     * throws if:
     *
     * - The value is not a valid CBOR value.
     * - The value is shorter than `size`.
     * - The value is longer than `size`.
     *
     * @param value Buffer in which to validate CBOR value.
     * @param offset Offset at which the value starts.
     * @param size Expected size of the value.
     */
    validate(value: Uint8Array, offset?: number, size?: number): void;
    decodeLevel(value: Uint8Array): unknown;
    /**
     * Decodes only one level of objects and arrays. Other values are decoded
     * completely.
     *
     * @returns One level of decoded CBOR value.
     */
    readLevel(): unknown;
    /**
     * Decodes primitive values, returns container values as `JsonPackValue`.
     *
     * @returns A primitive value, or CBOR container value as a blob.
     */
    readPrimitiveOrVal(): unknown | JsonPackValue;
    readAsValue(): JsonPackValue;
    readObjLevel(minor: number): Record<string, unknown>;
    readObjRawLevel(length: number): Record<string, unknown>;
    readObjIndefLevel(): Record<string, unknown>;
    readArrLevel(minor: number): unknown[];
    readArrRawLevel(length: number): unknown[];
    readArrIndefLevel(): unknown[];
    readHdr(expectedMajor: number): number;
    readStrHdr(): number;
    readObjHdr(): number;
    readArrHdr(): number;
    findKey(key: string): this;
    findIndex(index: number): this;
    find(path: Path): this;
}
