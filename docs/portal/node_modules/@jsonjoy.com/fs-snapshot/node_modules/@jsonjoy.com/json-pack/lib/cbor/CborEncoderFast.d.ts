import type { IWriter, IWriterGrowable } from '@jsonjoy.com/buffers/lib';
import type { BinaryJsonEncoder, StreamingBinaryJsonEncoder, TlvBinaryJsonEncoder } from '../types';
/**
 * Fast CBOR encoder supports only JSON values. Use regular `CborEncoder` if
 * you need ability to encode all CBOR value types.
 */
export declare class CborEncoderFast<W extends IWriter & IWriterGrowable = IWriter & IWriterGrowable> implements BinaryJsonEncoder, StreamingBinaryJsonEncoder, TlvBinaryJsonEncoder {
    readonly writer: W;
    constructor(writer?: W);
    encode(value: unknown): Uint8Array;
    writeAny(value: unknown): void;
    writeCbor(): void;
    writeEnd(): void;
    writeNull(): void;
    writeBoolean(bool: boolean): void;
    writeNumber(num: number): void;
    writeBigInt(int: bigint): void;
    writeBigUint(uint: bigint): void;
    writeBigSint(int: bigint): void;
    writeInteger(int: number): void;
    writeUInteger(uint: number): void;
    /** @deprecated Remove and use `writeNumber` instead. */
    encodeNumber(num: number): void;
    /** @deprecated Remove and use `writeInteger` instead. */
    encodeInteger(int: number): void;
    /** @deprecated */
    encodeUint(uint: number): void;
    encodeNint(int: number): void;
    writeFloat(float: number): void;
    writeBin(buf: Uint8Array): void;
    writeBinHdr(length: number): void;
    writeStr(str: string): void;
    writeStrHdr(length: number): void;
    writeAsciiStr(str: string): void;
    writeArr(arr: unknown[]): void;
    writeArrHdr(length: number): void;
    writeObj(obj: Record<string, unknown>): void;
    writeObjHdr(length: number): void;
    writeMapHdr(length: number): void;
    writeStartMap(): void;
    writeTag(tag: number, value: unknown): void;
    writeTagHdr(tag: number): void;
    writeTkn(value: number): void;
    writeStartStr(): void;
    writeStrChunk(str: string): void;
    writeEndStr(): void;
    writeStartBin(): void;
    writeBinChunk(buf: Uint8Array): void;
    writeEndBin(): void;
    writeStartArr(): void;
    writeArrChunk(item: unknown): void;
    writeEndArr(): void;
    writeStartObj(): void;
    writeObjChunk(key: string, value: unknown): void;
    writeEndObj(): void;
}
