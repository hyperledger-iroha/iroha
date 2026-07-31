import { Slice } from './Slice';
import type { IWriterGrowable, IWriter } from './types';
/**
 * Writer class provides an efficient way to encode binary data. It grows the
 * internal memory buffer automatically as more space is required. It is useful
 * in cases when it is not known in advance the size of memory needed.
 */
export declare class Writer implements IWriter, IWriterGrowable {
    allocSize: number;
    /** @ignore */
    uint8: Uint8Array;
    /** @ignore */
    view: DataView;
    /** @ignore */
    x0: number;
    /** @ignore */
    x: number;
    protected size: number;
    /**
     * @param allocSize Number of bytes to allocate at a time when buffer ends.
     */
    constructor(allocSize?: number);
    /** @ignore */
    protected grow(size: number): void;
    /**
     * Make sure the internal buffer has enough space to write the specified number
     * of bytes, otherwise resize the internal buffer to accommodate for more size.
     *
     * @param capacity Number of bytes.
     */
    ensureCapacity(capacity: number): void;
    /** @todo Consider renaming to "skip"? */
    move(capacity: number): void;
    reset(): void;
    /**
     * Allocates a new {@link ArrayBuffer}, useful when the underlying
     * {@link ArrayBuffer} cannot be shared between threads.
     *
     * @param size Size of memory to allocate.
     */
    newBuffer(size: number): void;
    /**
     * @returns Encoded memory buffer contents.
     */
    flush(): Uint8Array;
    flushSlice(): Slice;
    u8(char: number): void;
    u16(word: number): void;
    u32(dword: number): void;
    i32(dword: number): void;
    u64(qword: number | bigint): void;
    f64(float: number): void;
    u8u16(u8: number, u16: number): void;
    u8u32(u8: number, u32: number): void;
    u8u64(u8: number, u64: number | bigint): void;
    u8f32(u8: number, f32: number): void;
    u8f64(u8: number, f64: number): void;
    buf(buf: Uint8Array, length: number): void;
    /**
     * Encodes string as UTF-8. You need to call .ensureCapacity(str.length * 4)
     * before calling
     *
     * @param str String to encode as UTF-8.
     * @returns The number of bytes written
     */
    utf8(str: string): number;
    utf8Native(str: string): number;
    ascii(str: string): void;
}
