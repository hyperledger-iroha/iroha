import type { IReader, IReaderResettable } from './types';
export declare class Reader implements IReader, IReaderResettable {
    uint8: Uint8Array;
    view: DataView;
    x: number;
    end: number;
    constructor(uint8?: Uint8Array, view?: DataView, x?: number, end?: number);
    reset(uint8: Uint8Array): void;
    size(): number;
    /**
     * Get current byte value without advancing the cursor.
     */
    peek(): number;
    /**
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
     * Preferred over {@link buf} since it also provides a DataView and is much
     * faster to allocate a new {@link Slice} than a new {@link Uint8Array}.
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
}
