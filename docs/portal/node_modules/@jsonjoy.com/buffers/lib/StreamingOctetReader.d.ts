/**
 * A streaming reader which internally manages multiple chunks of
 * Uint8Array instances. For performance it does not merge the chunks into
 * a single Uint8Array instance. Instead it keeps track of the chunks and
 * reads across chunk boundaries as needed.
 */
export declare class StreamingOctetReader {
    protected readonly chunks: Uint8Array[];
    /** Total size of all chunks. */
    protected chunkSize: number;
    protected x: number;
    size(): number;
    push(chunk: Uint8Array): void;
    protected assertSize(size: number): void;
    u8(): number;
    u32(): number;
    copy(size: number, dst: Uint8Array, pos: number): void;
    copyXor(size: number, dst: Uint8Array, pos: number, mask: [number, number, number, number], maskIndex: number): void;
    buf(size: number): Uint8Array;
    bufXor(size: number, mask: [number, number, number, number], maskIndex: number): Uint8Array;
    skipUnsafe(n: number): void;
    skip(n: number): void;
    peek(): number;
    /**
     * Get current byte value without advancing the cursor.
     * @deprecated Use peek() instead.
     */
    peak(): number;
    utf8(length: number, mask: [number, number, number, number], maskIndex: number): string;
}
