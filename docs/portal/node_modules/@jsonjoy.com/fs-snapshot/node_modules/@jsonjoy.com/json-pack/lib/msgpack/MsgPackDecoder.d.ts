import { MsgPackDecoderFast } from './MsgPackDecoderFast';
import type { Path } from '@jsonjoy.com/json-pointer';
import type { Reader } from '@jsonjoy.com/buffers/lib/Reader';
/**
 * @category Decoder
 */
export declare class MsgPackDecoder extends MsgPackDecoderFast<Reader> {
    /**
     * Skips a whole JSON value and returns back the number of bytes
     * that value consumed.
     */
    skipAny(): number;
    /** @ignore */
    protected skipArr(size: number): number;
    /** @ignore */
    protected skipObj(size: number): number;
    readLevel(uint8: Uint8Array): unknown;
    protected valOneLevel(): unknown;
    /**
     * @ignore
     * @returns Returns a primitive value or {@link JsonPackValue} object, if the value
     *          is a "map" or an "arr".
     */
    protected primitive(): unknown;
    protected skip(length: number): number;
    /**
     * Throws if at given offset in a buffer there is an invalid MessagePack
     * value, or if the value does not span the exact length specified in `size`.
     * I.e. throws if:
     *
     * - The value is not a valid MessagePack value.
     * - The value is shorter than `size`.
     * - The value is longer than `size`.
     *
     * @param value Buffer in which to validate MessagePack value.
     * @param offset Offset at which the value starts.
     * @param size Expected size of the value.
     */
    validate(value: Uint8Array, offset?: number, size?: number): void;
    readObjHdr(): number;
    readStrHdr(): number;
    findKey(key: string): this;
    readArrHdr(): number;
    findIndex(index: number): this;
    find(path: Path): this;
}
