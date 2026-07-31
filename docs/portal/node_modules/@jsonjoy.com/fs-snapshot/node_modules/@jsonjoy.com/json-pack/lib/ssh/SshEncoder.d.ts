import { JsonPackMpint } from '../JsonPackMpint';
import type { IWriter, IWriterGrowable } from '@jsonjoy.com/buffers/lib';
import type { BinaryJsonEncoder } from '../types';
/**
 * SSH 2.0 binary encoder for SSH protocol data types.
 * Implements SSH binary encoding according to RFC 4251.
 *
 * Key SSH encoding principles:
 * - Multi-byte quantities are transmitted in big-endian byte order (network byte order)
 * - Strings are length-prefixed with uint32
 * - No padding is used (unlike XDR)
 */
export declare class SshEncoder implements BinaryJsonEncoder {
    readonly writer: IWriter & IWriterGrowable;
    constructor(writer: IWriter & IWriterGrowable);
    encode(value: unknown): Uint8Array;
    /**
     * Called when the encoder encounters a value that it does not know how to encode.
     */
    writeUnknown(value: unknown): void;
    writeAny(value: unknown): void;
    /**
     * SSH doesn't have a null type, but we provide it for interface compatibility.
     */
    writeNull(): void;
    /**
     * Writes an SSH boolean value as a single byte.
     * The value 0 represents FALSE, and the value 1 represents TRUE.
     */
    writeBoolean(bool: boolean): void;
    /**
     * Writes an SSH byte value (8-bit).
     */
    writeByte(byte: number): void;
    /**
     * Writes an SSH uint32 value in big-endian format.
     */
    writeUint32(uint: number): void;
    /**
     * Writes an SSH uint64 value in big-endian format.
     */
    writeUint64(uint: number | bigint): void;
    /**
     * Writes an SSH string as binary data (Uint8Array).
     * Format: uint32 length + data bytes (no padding).
     */
    writeBinStr(data: Uint8Array): void;
    /**
     * Writes an SSH string with UTF-8 encoding.
     * Format: uint32 length + UTF-8 bytes (no padding).
     */
    writeStr(str: string): void;
    /**
     * Writes an SSH string with ASCII encoding.
     * Format: uint32 length + ASCII bytes (no padding).
     */
    writeAsciiStr(str: string): void;
    /**
     * Writes an SSH mpint (multiple precision integer).
     * Format: uint32 length + data bytes in two's complement format, MSB first.
     */
    writeMpint(mpint: JsonPackMpint): void;
    /**
     * Writes an SSH name-list.
     * Format: uint32 length + comma-separated names.
     */
    writeNameList(names: string[]): void;
    /**
     * Generic number writing - writes as uint32 by default
     */
    writeNumber(num: number): void;
    /**
     * Writes an integer value as uint32
     */
    writeInteger(int: number): void;
    /**
     * Writes an unsigned integer value as uint32
     */
    writeUInteger(uint: number): void;
    /**
     * Writes a float value - SSH doesn't support floats
     */
    writeFloat(float: number): void;
    /**
     * Writes binary data as SSH string
     */
    writeBin(buf: Uint8Array): void;
    /**
     * Writes arrays - not supported in base SSH protocol
     */
    writeArr(arr: unknown[]): void;
    /**
     * Writes objects - not supported in base SSH protocol
     */
    writeObj(obj: Record<string, unknown>): void;
}
