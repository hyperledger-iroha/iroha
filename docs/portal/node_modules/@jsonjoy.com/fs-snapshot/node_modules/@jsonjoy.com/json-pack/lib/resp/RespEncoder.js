"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.RespEncoder = void 0;
const Writer_1 = require("@jsonjoy.com/buffers/lib/Writer");
const utf8_1 = require("@jsonjoy.com/util/lib/strings/utf8");
const extensions_1 = require("./extensions");
const JsonPackExtension_1 = require("../JsonPackExtension");
const REG_RN = /[\r\n]/;
const isSafeInteger = Number.isSafeInteger;
/**
 * Implements RESP3 encoding.
 */
class RespEncoder {
    constructor(writer = new Writer_1.Writer()) {
        this.writer = writer;
    }
    encode(value) {
        this.writeAny(value);
        return this.writer.flush();
    }
    encodeToSlice(value) {
        this.writeAny(value);
        return this.writer.flushSlice();
    }
    writeAny(value) {
        switch (typeof value) {
            case 'number':
                return this.writeNumber(value);
            case 'string':
                return this.writeStr(value);
            case 'boolean':
                return this.writeBoolean(value);
            case 'object': {
                if (!value)
                    return this.writeNull();
                if (value instanceof Array)
                    return this.writeArr(value);
                if (value instanceof Uint8Array)
                    return this.writeBin(value);
                if (value instanceof Error)
                    return this.writeErr(value.message);
                if (value instanceof Set)
                    return this.writeSet(value);
                if (value instanceof JsonPackExtension_1.JsonPackExtension) {
                    if (value instanceof extensions_1.RespPush)
                        return this.writePush(value.val);
                    if (value instanceof extensions_1.RespVerbatimString)
                        return this.writeVerbatimStr('txt', value.val);
                    if (value instanceof extensions_1.RespAttributes)
                        return this.writeAttr(value.val);
                }
                return this.writeObj(value);
            }
            case 'undefined':
                return this.writeUndef();
            case 'bigint':
                return this.writeBigInt(value);
            default:
                return this.writeUnknown(value);
        }
    }
    writeLength(length) {
        const writer = this.writer;
        if (length < 100) {
            if (length < 10) {
                writer.u8(length + 48);
                return;
            }
            const octet1 = length % 10;
            const octet2 = (length - octet1) / 10;
            writer.u16(((octet2 + 48) << 8) + octet1 + 48);
            return;
        }
        let digits = 1;
        let pow = 10;
        while (length >= pow) {
            digits++;
            pow *= 10;
        }
        writer.ensureCapacity(digits);
        const uint8 = writer.uint8;
        const x = writer.x;
        const newX = x + digits;
        let i = newX - 1;
        while (i >= x) {
            const remainder = length % 10;
            uint8[i--] = remainder + 48;
            length = (length - remainder) / 10;
        }
        writer.x = newX;
    }
    encodeCmd(args) {
        this.writeCmd(args);
        return this.writer.flush();
    }
    writeCmd(args) {
        const length = args.length;
        this.writeArrHdr(length);
        for (let i = 0; i < length; i++) {
            const arg = args[i];
            if (arg instanceof Uint8Array)
                this.writeBin(arg);
            else
                this.writeBulkStrAscii(arg + '');
        }
    }
    encodeCmdUtf8(args) {
        this.writeCmdUtf8(args);
        return this.writer.flush();
    }
    writeCmdUtf8(args) {
        const length = args.length;
        this.writeArrHdr(length);
        for (let i = 0; i < length; i++)
            this.writeArgUtf8(args[i]);
    }
    writeArgUtf8(arg) {
        if (arg instanceof Uint8Array)
            return this.writeBin(arg);
        else
            this.writeBulkStr(arg + '');
    }
    writeNull() {
        this.writer.u8u16(95 /* RESP.NULL */, // _
        3338 /* RESP.RN */);
    }
    writeNullStr() {
        this.writer.u8u32(36 /* RESP.STR_BULK */, // $
        45 * 0x1000000 + // -
            49 * 0x10000 + // 1
            3338 /* RESP.RN */);
    }
    writeNullArr() {
        this.writer.u8u32(42 /* RESP.ARR */, // *
        45 * 0x1000000 + // -
            49 * 0x10000 + // 1
            3338 /* RESP.RN */);
    }
    writeBoolean(bool) {
        this.writer.u32(bool
            ? 35 /* RESP.BOOL */ * 0x1000000 + // #
                116 * 0x10000 + // t
                3338 /* RESP.RN */ // \r\n
            : 35 /* RESP.BOOL */ * 0x1000000 + // #
                102 * 0x10000 + // f
                3338 /* RESP.RN */);
    }
    writeNumber(num) {
        if (isSafeInteger(num))
            this.writeInteger(num);
        else if (typeof num === 'bigint')
            this.writeBigInt(num);
        else
            this.writeFloat(num);
    }
    writeBigInt(int) {
        const writer = this.writer;
        writer.u8(40 /* RESP.BIG */); // (
        writer.ascii(int + '');
        writer.u16(3338 /* RESP.RN */); // \r\n
    }
    writeInteger(int) {
        const writer = this.writer;
        writer.u8(58 /* RESP.INT */); // :
        writer.ascii(int + '');
        writer.u16(3338 /* RESP.RN */); // \r\n
    }
    writeUInteger(uint) {
        this.writeInteger(uint);
    }
    writeFloat(float) {
        const writer = this.writer;
        writer.u8(44 /* RESP.FLOAT */); // ,
        switch (float) {
            case Infinity:
                writer.u8u16(105, // i
                (110 << 8) | // n
                    102);
                break;
            case -Infinity:
                writer.u32((45 * 0x1000000 + // -
                    105 * 0x10000 + // i
                    (110 << 8)) | // n
                    102);
                break;
            default:
                if (float !== float)
                    writer.u8u16(110, // n
                    (97 << 8) | // a
                        110);
                else
                    writer.ascii(float + '');
                break;
        }
        writer.u16(3338 /* RESP.RN */); // \r\n
    }
    writeBin(buf) {
        const writer = this.writer;
        const length = buf.length;
        writer.u8(36 /* RESP.STR_BULK */); // $
        this.writeLength(length);
        writer.u16(3338 /* RESP.RN */); // \r\n
        writer.buf(buf, length);
        writer.u16(3338 /* RESP.RN */); // \r\n
    }
    writeBinHdr(length) {
        throw new Error('Not implemented');
        // Because then we also need `.writeBinBody()` which would emit trailing `\r\n`.
    }
    writeStr(str) {
        const length = str.length;
        if (length < 64 && !REG_RN.test(str))
            this.writeSimpleStr(str);
        else
            this.writeVerbatimStr('txt', str);
    }
    writeStrHdr(length) {
        throw new Error('Not implemented');
        // Because then we also need `.writeBinBody()` which would emit trailing `\r\n`.
    }
    writeSimpleStr(str) {
        const writer = this.writer;
        writer.u8(43 /* RESP.STR_SIMPLE */); // +
        writer.ensureCapacity(str.length << 2);
        writer.utf8(str);
        writer.u16(3338 /* RESP.RN */); // \r\n
    }
    writeSimpleStrAscii(str) {
        const writer = this.writer;
        writer.u8(43 /* RESP.STR_SIMPLE */); // +
        writer.ascii(str);
        writer.u16(3338 /* RESP.RN */); // \r\n
    }
    writeBulkStr(str) {
        const writer = this.writer;
        const size = (0, utf8_1.utf8Size)(str);
        writer.u8(36 /* RESP.STR_BULK */); // $
        this.writeLength(size);
        writer.u16(3338 /* RESP.RN */); // \r\n
        writer.ensureCapacity(size);
        writer.utf8(str);
        writer.u16(3338 /* RESP.RN */); // \r\n
    }
    writeBulkStrAscii(str) {
        const writer = this.writer;
        writer.u8(36 /* RESP.STR_BULK */); // $
        this.writeLength(str.length);
        writer.u16(3338 /* RESP.RN */); // \r\n
        writer.ascii(str);
        writer.u16(3338 /* RESP.RN */); // \r\n
    }
    writeAsciiStr(str) {
        const isSimple = !REG_RN.test(str);
        if (isSimple)
            this.writeSimpleStr(str);
        else
            this.writeBulkStrAscii(str);
    }
    writeVerbatimStr(encoding, str) {
        const writer = this.writer;
        const size = (0, utf8_1.utf8Size)(str);
        writer.u8(61 /* RESP.STR_VERBATIM */); // =
        this.writeLength(size + 4);
        writer.u16(3338 /* RESP.RN */); // \r\n
        writer.u32(encoding.charCodeAt(0) * 0x1000000 + // t
            (encoding.charCodeAt(1) << 16) + // x
            (encoding.charCodeAt(2) << 8) + // t
            58);
        writer.ensureCapacity(size);
        writer.utf8(str);
        writer.u16(3338 /* RESP.RN */); // \r\n
    }
    writeErr(str) {
        if (str.length < 64 && !REG_RN.test(str))
            this.writeSimpleErr(str);
        else
            this.writeBulkErr(str);
    }
    writeSimpleErr(str) {
        const writer = this.writer;
        writer.u8(45 /* RESP.ERR_SIMPLE */); // -
        writer.ensureCapacity(str.length << 2);
        writer.utf8(str);
        writer.u16(3338 /* RESP.RN */); // \r\n
    }
    writeBulkErr(str) {
        const writer = this.writer;
        const size = (0, utf8_1.utf8Size)(str);
        writer.u8(33 /* RESP.ERR_BULK */); // !
        this.writeLength(size);
        writer.u16(3338 /* RESP.RN */); // \r\n
        writer.ensureCapacity(size);
        writer.utf8(str);
        writer.u16(3338 /* RESP.RN */); // \r\n
    }
    writeArr(arr) {
        const writer = this.writer;
        const length = arr.length;
        writer.u8(42 /* RESP.ARR */); // *
        this.writeLength(length);
        writer.u16(3338 /* RESP.RN */); // \r\n
        for (let i = 0; i < length; i++)
            this.writeAny(arr[i]);
    }
    writeArrHdr(length) {
        const writer = this.writer;
        writer.u8(42 /* RESP.ARR */); // *
        this.writeLength(length);
        writer.u16(3338 /* RESP.RN */); // \r\n
    }
    writeObj(obj) {
        const writer = this.writer;
        const keys = Object.keys(obj);
        const length = keys.length;
        writer.u8(37 /* RESP.OBJ */); // %
        this.writeLength(length);
        writer.u16(3338 /* RESP.RN */); // \r\n
        for (let i = 0; i < length; i++) {
            const key = keys[i];
            this.writeStr(key);
            this.writeAny(obj[key]);
        }
    }
    writeObjHdr(length) {
        const writer = this.writer;
        writer.u8(37 /* RESP.OBJ */); // %
        this.writeLength(length);
        writer.u16(3338 /* RESP.RN */); // \r\n
    }
    writeAttr(obj) {
        const writer = this.writer;
        const keys = Object.keys(obj);
        const length = keys.length;
        writer.u8(124 /* RESP.ATTR */); // |
        this.writeLength(length);
        writer.u16(3338 /* RESP.RN */); // \r\n
        for (let i = 0; i < length; i++) {
            const key = keys[i];
            this.writeStr(key);
            this.writeAny(obj[key]);
        }
    }
    writeSet(set) {
        const writer = this.writer;
        const length = set.size;
        writer.u8(126 /* RESP.SET */); // ~
        this.writeLength(length);
        writer.u16(3338 /* RESP.RN */); // \r\n
        for (let i = 0; i < length; i++)
            set.forEach((value) => this.writeAny(value));
    }
    writePush(elements) {
        const writer = this.writer;
        const length = elements.length;
        writer.u8(62 /* RESP.PUSH */); // >
        this.writeLength(length);
        writer.u16(3338 /* RESP.RN */); // \r\n
        for (let i = 0; i < length; i++)
            this.writeAny(elements[i]);
    }
    /**
     * Called when the encoder encounters a value that it does not know how to encode.
     *
     * @param value Some JavaScript value.
     */
    writeUnknown(value) {
        this.writeNull();
    }
    writeUndef() {
        this.writeNull();
    }
    writeRn() {
        this.writer.u16(3338 /* RESP.RN */); // \r\n
    }
    // ---------------------------------------------------------- Stream encoding
    writeStartStr() {
        this.writer.u32(36 /* RESP.STR_BULK */ * 0x1000000 + // $
            (63 << 16) + // ?
            3338 /* RESP.RN */);
    }
    writeStrChunk(str) {
        const writer = this.writer;
        writer.u8(59); // ;
        const size = (0, utf8_1.utf8Size)(str);
        this.writeLength(size);
        writer.u16(3338 /* RESP.RN */); // \r\n
        writer.ensureCapacity(size);
        writer.utf8(str);
        writer.u16(3338 /* RESP.RN */); // \r\n
    }
    writeEndStr() {
        this.writer.u32(59 * 0x1000000 + // ;
            (48 << 16) + // 0
            3338 /* RESP.RN */);
    }
    writeStartBin() {
        this.writer.u32(36 * 0x1000000 + // $
            (63 << 16) + // ?
            3338 /* RESP.RN */);
    }
    writeBinChunk(buf) {
        const writer = this.writer;
        const length = buf.length;
        writer.u8(59); // ;
        this.writeLength(length);
        writer.u16(3338 /* RESP.RN */); // \r\n
        writer.buf(buf, length);
        writer.u16(3338 /* RESP.RN */); // \r\n
    }
    writeEndBin() {
        this.writer.u32(59 * 0x1000000 + // ;
            (48 << 16) + // 0
            3338 /* RESP.RN */);
    }
    writeStartArr() {
        this.writer.u32(42 /* RESP.ARR */ * 0x1000000 + // *
            (63 << 16) + // ?
            3338 /* RESP.RN */);
    }
    writeArrChunk(item) {
        this.writeAny(item);
    }
    writeEndArr() {
        this.writer.u8u16(46, // .
        3338 /* RESP.RN */);
    }
    writeStartObj() {
        this.writer.u32(37 * 0x1000000 + // %
            (63 << 16) + // ?
            3338 /* RESP.RN */);
    }
    writeObjChunk(key, value) {
        this.writeStr(key);
        this.writeAny(value);
    }
    writeEndObj() {
        this.writer.u8u16(46, // .
        3338 /* RESP.RN */);
    }
}
exports.RespEncoder = RespEncoder;
//# sourceMappingURL=RespEncoder.js.map