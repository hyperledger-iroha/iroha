import type { IWriter, IWriterGrowable } from '@jsonjoy.com/buffers/lib';
import type { JsonPackExtension } from '../JsonPackExtension';
import type { BinaryJsonEncoder, TlvBinaryJsonEncoder } from '../types';
import type { IMessagePackEncoder } from './types';
/**
 * @category Encoder
 */
export declare class MsgPackEncoderFast<W extends IWriter & IWriterGrowable = IWriter & IWriterGrowable> implements IMessagePackEncoder, BinaryJsonEncoder, TlvBinaryJsonEncoder {
    readonly writer: W;
    constructor(writer?: W);
    /**
     * Use this method to encode a JavaScript document into MessagePack format.
     *
     * @param json JSON value to encode.
     * @returns Encoded memory buffer with MessagePack contents.
     */
    encode(json: unknown): Uint8Array;
    /** @deprecated */
    encodeAny(json: unknown): void;
    writeAny(value: unknown): void;
    /** @deprecated */
    protected encodeFloat64(num: number): void;
    writeNull(): void;
    writeFloat(float: number): void;
    u32(num: number): void;
    n32(num: number): void;
    /** @deprecated */
    encodeNumber(num: number): void;
    writeNumber(num: number): void;
    writeInteger(int: number): void;
    writeUInteger(uint: number): void;
    encodeNull(): void;
    encodeTrue(): void;
    encodeFalse(): void;
    /** @deprecated */
    encodeBoolean(bool: boolean): void;
    writeBoolean(bool: boolean): void;
    /** @deprecated */
    encodeStringHeader(length: number): void;
    writeStrHdr(length: number): void;
    /** @deprecated */
    encodeString(str: string): void;
    writeStr(str: string): void;
    /** @deprecated */
    encodeAsciiString(str: string): void;
    writeAsciiStr(str: string): void;
    /** @deprecated */
    encodeArrayHeader(length: number): void;
    /** @deprecated */
    encodeArray(arr: unknown[]): void;
    writeArrHdr(length: number): void;
    writeArr(arr: unknown[]): void;
    /** @deprecated */
    encodeObjectHeader(length: number): void;
    /** @deprecated */
    encodeObject(obj: Record<string, unknown>): void;
    writeObjHdr(length: number): void;
    writeObj(obj: Record<string, unknown>): void;
    encodeExtHeader(type: number, length: number): void;
    encodeExt(ext: JsonPackExtension): void;
    /** @deprecated */
    encodeBinaryHeader(length: number): void;
    /** @deprecated */
    encodeBinary(buf: Uint8Array): void;
    writeBinHdr(length: number): void;
    writeBin(buf: Uint8Array): void;
}
