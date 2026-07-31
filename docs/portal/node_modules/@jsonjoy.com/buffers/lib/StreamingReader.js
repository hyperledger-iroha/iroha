"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.StreamingReader = void 0;
const Writer_1 = require("./Writer");
const decodeUtf8_1 = require("./utf8/decodeUtf8");
const Reader_1 = require("./Reader");
class StreamingReader {
    constructor(allocSize = 16 * 1024) {
        /**
         * Offset from the start of the buffer (x0 in Writer).
         */
        this.dx = 0;
        this.writer = new Writer_1.Writer(allocSize);
    }
    /**
     * Returns the number of bytes remaining in the buffer.
     */
    size() {
        return this.writer.x - this.x;
    }
    /**
     * Assert that there is enough data in the buffer to read `size` bytes.
     *
     * @param size Number of bytes to read.
     */
    assertSize(size) {
        if (size > this.size())
            throw new RangeError('OUT_OF_BOUNDS');
    }
    /**
     * Add a chunk of data to be decoded. The chunk is copied into the
     * internal buffer, so you can reuse the chunk after calling this method; or
     * this chunk can be neutered by the caller.
     *
     * @param uint8 `Uint8Array` chunk of data to be decoded.
     */
    push(uint8) {
        this.writer.buf(uint8, uint8.length);
    }
    /**
     * Mark the current position as consumed. This will free up memory
     * for reuse.
     */
    consume() {
        this.writer.x0 += this.dx;
        this.dx = 0;
    }
    // ------------------------------------------------------------------ IReader
    get uint8() {
        return this.writer.uint8;
    }
    get view() {
        return this.writer.view;
    }
    get x() {
        return this.writer.x0 + this.dx;
    }
    set x(x) {
        this.dx = x - this.writer.x0;
    }
    peek() {
        this.assertSize(1);
        return this.view.getUint8(this.x);
    }
    /**
     * Get current byte value without advancing the cursor.
     * @deprecated Use peek() instead.
     */
    peak() {
        return this.peek();
    }
    skip(length) {
        this.assertSize(length);
        this.x += length;
    }
    buf(size = this.size()) {
        this.assertSize(size);
        const end = this.x + size;
        const bin = this.uint8.subarray(this.x, end);
        this.x = end;
        return bin;
    }
    subarray(start = 0, end) {
        const x = this.x;
        const actualStart = x + start;
        const actualEnd = typeof end === 'number' ? x + end : this.size() + x - start;
        return this.uint8.subarray(actualStart, actualEnd);
    }
    /**
     * Creates a new {@link Reader} that references the same underlying memory
     * buffer. But with independent cursor and end.
     *
     * @param start Start offset relative to the current cursor position.
     * @param end End offset relative to the current cursor position.
     * @returns A new {@link Reader} instance.
     */
    slice(start = 0, end) {
        const x = this.x;
        const actualStart = x + start;
        const actualEnd = typeof end === 'number' ? x + end : this.size() + x - start;
        return new Reader_1.Reader(this.uint8, this.view, actualStart, actualEnd);
    }
    /**
     * Similar to {@link slice} but also advances the cursor. Returns a new
     * {@link Reader} that references the same underlying memory buffer, starting
     * from the current cursor position.
     *
     * @param size Number of bytes to cut from the current position.
     * @returns A new {@link Reader} instance.
     */
    cut(size = this.size()) {
        const slice = this.slice(0, size);
        this.skip(size);
        return slice;
    }
    u8() {
        this.assertSize(1);
        return this.view.getUint8(this.x++);
    }
    i8() {
        this.assertSize(1);
        return this.view.getInt8(this.x++);
    }
    u16() {
        this.assertSize(2);
        const num = this.view.getUint16(this.x);
        this.x += 2;
        return num;
    }
    i16() {
        this.assertSize(2);
        const num = this.view.getInt16(this.x);
        this.x += 2;
        return num;
    }
    u32() {
        this.assertSize(4);
        const num = this.view.getUint32(this.x);
        this.x += 4;
        return num;
    }
    i32() {
        this.assertSize(4);
        const num = this.view.getInt32(this.x);
        this.x += 4;
        return num;
    }
    u64() {
        this.assertSize(8);
        const num = this.view.getBigUint64(this.x);
        this.x += 8;
        return num;
    }
    i64() {
        this.assertSize(8);
        const num = this.view.getBigInt64(this.x);
        this.x += 8;
        return num;
    }
    f32() {
        this.assertSize(4);
        const pos = this.x;
        this.x += 4;
        return this.view.getFloat32(pos);
    }
    f64() {
        this.assertSize(8);
        const pos = this.x;
        this.x += 8;
        return this.view.getFloat64(pos);
    }
    utf8(size) {
        this.assertSize(size);
        const start = this.x;
        this.x += size;
        return (0, decodeUtf8_1.decodeUtf8)(this.uint8, start, size);
    }
    ascii(length) {
        this.assertSize(length);
        const uint8 = this.uint8;
        let str = '';
        const end = this.x + length;
        for (let i = this.x; i < end; i++)
            str += String.fromCharCode(uint8[i]);
        this.x = end;
        return str;
    }
    // -------------------------------------------------------- IReaderResettable
    reset(uint8) {
        this.dx = 0;
        this.writer.reset();
        this.push(uint8);
    }
}
exports.StreamingReader = StreamingReader;
//# sourceMappingURL=StreamingReader.js.map