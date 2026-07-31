import { Writer } from './Writer';
import type { IReader, IReaderResettable } from './types';
import { Reader } from './Reader';
export declare class StreamingReader implements IReader, IReaderResettable {
    protected readonly writer: Writer;
    /**
     * Offset from the start of the buffer (x0 in Writer).
     */
    protected dx: number;
    constructor(allocSize?: number);
    /**
     * Returns the number of bytes remaining in the buffer.
     */
    size(): number;
    /**
     * Assert that there is enough data in the buffer to read `size` bytes.
     *
     * @param size Number of bytes to read.
     */
    protected assertSize(size: number): void;
    /**
     * Add a chunk of data to be decoded. The chunk is copied into the
     * internal buffer, so you can reuse the chunk after calling this method; or
     * this chunk can be neutered by the caller.
     *
     * @param uint8 `Uint8Array` chunk of data to be decoded.
     */
    push(uint8: Uint8Array): void;
    /**
     * Mark the current position as consumed. This will free up memory
     * for reuse.
     */
    consume(): void;
    get uint8(): Uint8Array;
    get view(): DataView;
    get x(): number;
    set x(x: number);
    peek(): number;
    /**
     * Get current byte value without advancing the cursor.
     * @deprecated Use peek() instead.
     */
    peak(): number;
    skip(length: number): void;
    buf(size?: number): Uint8Array;
    subarray(start?: number, end?: number): Uint8Array;
    /**
     * Creates a new {@link Reader} that references the same underlying memory
     * buffer. But with independent cursor and end.
     *
     * @param start Start offset relative to the current cursor position.
     * @param end End offset relative to the current cursor position.
     * @returns A new {@link Reader} instance.
     */
    slice(start?: number, end?: number): Reader;
    /**
     * Similar to {@link slice} but also advances the cursor. Returns a new
     * {@link Reader} that references the same underlying memory buffer, starting
     * from the current cursor position.
     *
     * @param size Number of bytes to cut from the current position.
     * @returns A new {@link Reader} instance.
     */
    cut(size?: number): Reader;
    u8(): number;
    i8(): number;
    u16(): number;
    i16(): number;
    u32(): number;
    i32(): number;
    u64(): bigint;
    i64(): bigint;
    f32(): number;
    f64(): number;
    utf8(size: number): string;
    ascii(length: number): string;
    reset(uint8: Uint8Array): void;
}
