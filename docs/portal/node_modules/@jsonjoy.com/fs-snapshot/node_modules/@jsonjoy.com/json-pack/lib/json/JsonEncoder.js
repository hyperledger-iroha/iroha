"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.JsonEncoder = void 0;
const toBase64Bin_1 = require("@jsonjoy.com/base64/lib/toBase64Bin");
class JsonEncoder {
    constructor(writer) {
        this.writer = writer;
    }
    encode(value) {
        const writer = this.writer;
        writer.reset();
        this.writeAny(value);
        return writer.flush();
    }
    /**
     * Called when the encoder encounters a value that it does not know how to encode.
     *
     * @param value Some JavaScript value.
     */
    writeUnknown(value) {
        this.writeNull();
    }
    writeAny(value) {
        switch (typeof value) {
            case 'boolean':
                return this.writeBoolean(value);
            case 'number':
                return this.writeNumber(value);
            case 'string':
                return this.writeStr(value);
            case 'object': {
                if (value === null)
                    return this.writeNull();
                const constr = value.constructor;
                switch (constr) {
                    case Object:
                        return this.writeObj(value);
                    case Array:
                        return this.writeArr(value);
                    case Uint8Array:
                        return this.writeBin(value);
                    default:
                        if (value instanceof Uint8Array)
                            return this.writeBin(value);
                        if (Array.isArray(value))
                            return this.writeArr(value);
                        return this.writeUnknown(value);
                }
            }
            case 'undefined': {
                return this.writeUndef();
            }
            default:
                return this.writeUnknown(value);
        }
    }
    writeNull() {
        this.writer.u32(0x6e756c6c); // null
    }
    writeUndef() {
        const writer = this.writer;
        const length = 35;
        writer.ensureCapacity(length);
        // Write: "data:application/cbor,base64;9w=="
        const view = writer.view;
        let x = writer.x;
        view.setUint32(x, 577003892); // "dat
        x += 4;
        view.setUint32(x, 1631215984); // a:ap
        x += 4;
        view.setUint32(x, 1886153059); // plic
        x += 4;
        view.setUint32(x, 1635019119); // atio
        x += 4;
        view.setUint32(x, 1848599394); // n/cb
        x += 4;
        view.setUint32(x, 1869753442); // or,b
        x += 4;
        view.setUint32(x, 1634952502); // ase6
        x += 4;
        view.setUint32(x, 876296567); // 4;9w
        x += 4;
        view.setUint16(x, 15677); // ==
        x += 2;
        writer.uint8[x++] = 0x22; // "
        writer.x = x;
    }
    writeBoolean(bool) {
        if (bool)
            this.writer.u32(0x74727565); // true
        else
            this.writer.u8u32(0x66, 0x616c7365); // false
    }
    writeNumber(num) {
        const str = num.toString();
        this.writer.ascii(str);
    }
    writeInteger(int) {
        this.writeNumber(int >> 0 === int ? int : Math.trunc(int));
    }
    writeUInteger(uint) {
        this.writeInteger(uint < 0 ? -uint : uint);
    }
    writeFloat(float) {
        this.writeNumber(float);
    }
    writeBin(buf) {
        const writer = this.writer;
        const length = buf.length;
        writer.ensureCapacity(38 + 3 + (length << 1));
        // Write: "data:application/octet-stream;base64, - 22 64 61 74 61 3a 61 70 70 6c 69 63 61 74 69 6f 6e 2f 6f 63 74 65 74 2d 73 74 72 65 61 6d 3b 62 61 73 65 36 34 2c
        const view = writer.view;
        let x = writer.x;
        view.setUint32(x, 577003892); // "dat
        x += 4;
        view.setUint32(x, 1631215984); // a:ap
        x += 4;
        view.setUint32(x, 1886153059); // plic
        x += 4;
        view.setUint32(x, 1635019119); // atio
        x += 4;
        view.setUint32(x, 1848602467); // n/oc
        x += 4;
        view.setUint32(x, 1952805933); // tet-
        x += 4;
        view.setUint32(x, 1937011301); // stre
        x += 4;
        view.setUint32(x, 1634548578); // am;b
        x += 4;
        view.setUint32(x, 1634952502); // ase6
        x += 4;
        view.setUint16(x, 13356); // 4,
        x += 2;
        x = (0, toBase64Bin_1.toBase64Bin)(buf, 0, length, view, x);
        writer.uint8[x++] = 0x22; // "
        writer.x = x;
    }
    writeStr(str) {
        const writer = this.writer;
        const length = str.length;
        writer.ensureCapacity(length * 4 + 2);
        if (length < 256) {
            const startX = writer.x;
            let x = startX;
            const uint8 = writer.uint8;
            uint8[x++] = 0x22; // "
            for (let i = 0; i < length; i++) {
                const code = str.charCodeAt(i);
                switch (code) {
                    case 34: // "
                    case 92: // \
                        uint8[x++] = 0x5c; // \
                        break;
                }
                if (code < 32 || code > 126) {
                    writer.x = startX;
                    const jsonStr = JSON.stringify(str);
                    writer.ensureCapacity(jsonStr.length * 4 + 4);
                    writer.utf8(jsonStr);
                    return;
                }
                else
                    uint8[x++] = code;
            }
            uint8[x++] = 0x22; // "
            writer.x = x;
            return;
        }
        const jsonStr = JSON.stringify(str);
        writer.ensureCapacity(jsonStr.length * 4 + 4);
        writer.utf8(jsonStr);
    }
    writeAsciiStr(str) {
        const length = str.length;
        const writer = this.writer;
        writer.ensureCapacity(length * 2 + 2);
        const uint8 = writer.uint8;
        let x = writer.x;
        uint8[x++] = 0x22; // "
        for (let i = 0; i < length; i++) {
            const code = str.charCodeAt(i);
            switch (code) {
                case 34: // "
                case 92: // \
                    uint8[x++] = 0x5c; // \
                    break;
            }
            uint8[x++] = code;
        }
        uint8[x++] = 0x22; // "
        writer.x = x;
    }
    writeArr(arr) {
        const writer = this.writer;
        writer.u8(0x5b); // [
        const length = arr.length;
        const last = length - 1;
        for (let i = 0; i < last; i++) {
            this.writeAny(arr[i]);
            writer.u8(0x2c); // ,
        }
        if (last >= 0)
            this.writeAny(arr[last]);
        writer.u8(0x5d); // ]
    }
    writeArrSeparator() {
        this.writer.u8(0x2c); // ,
    }
    writeObj(obj) {
        const writer = this.writer;
        const keys = Object.keys(obj);
        const length = keys.length;
        if (!length)
            return writer.u16(0x7b7d); // {}
        writer.u8(0x7b); // {
        for (let i = 0; i < length; i++) {
            const key = keys[i];
            const value = obj[key];
            this.writeStr(key);
            writer.u8(0x3a); // :
            this.writeAny(value);
            writer.u8(0x2c); // ,
        }
        writer.uint8[writer.x - 1] = 0x7d; // }
    }
    writeObjSeparator() {
        this.writer.u8(0x2c); // ,
    }
    writeObjKeySeparator() {
        this.writer.u8(0x3a); // :
    }
    // ------------------------------------------------------- Streaming encoding
    writeStartStr() {
        throw new Error('Method not implemented.');
    }
    writeStrChunk(str) {
        throw new Error('Method not implemented.');
    }
    writeEndStr() {
        throw new Error('Method not implemented.');
    }
    writeStartBin() {
        throw new Error('Method not implemented.');
    }
    writeBinChunk(buf) {
        throw new Error('Method not implemented.');
    }
    writeEndBin() {
        throw new Error('Method not implemented.');
    }
    writeStartArr() {
        this.writer.u8(0x5b); // [
    }
    writeArrChunk(item) {
        throw new Error('Method not implemented.');
    }
    writeEndArr() {
        this.writer.u8(0x5d); // ]
    }
    writeStartObj() {
        this.writer.u8(0x7b); // {
    }
    writeObjChunk(key, value) {
        throw new Error('Method not implemented.');
    }
    writeEndObj() {
        this.writer.u8(0x7d); // }
    }
}
exports.JsonEncoder = JsonEncoder;
//# sourceMappingURL=JsonEncoder.js.map