"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.BsonDecoder = void 0;
const Reader_1 = require("@jsonjoy.com/buffers/lib/Reader");
const values_1 = require("./values");
class BsonDecoder {
    constructor(reader = new Reader_1.Reader()) {
        this.reader = reader;
    }
    read(uint8) {
        this.reader.reset(uint8);
        return this.readDocument();
    }
    decode(uint8) {
        this.reader.reset(uint8);
        return this.readDocument();
    }
    readAny() {
        return this.readDocument();
    }
    readDocument() {
        const reader = this.reader;
        const documentSize = reader.view.getInt32(reader.x, true); // true = little-endian
        reader.x += 4;
        const startPos = reader.x; // Position after reading the size
        const endPos = startPos + documentSize - 4 - 1; // End position before the terminating null
        const obj = {};
        while (reader.x < endPos) {
            const elementType = reader.u8();
            if (elementType === 0)
                break; // End of document
            const key = this.readCString();
            const value = this.readElementValue(elementType);
            obj[key] = value;
        }
        // Skip to the end of document (including the terminating null if we haven't read it)
        if (reader.x <= endPos) {
            reader.x = startPos + documentSize - 4; // Move to just after the terminating null
        }
        return obj;
    }
    readCString() {
        const reader = this.reader;
        const uint8 = reader.uint8;
        const x = reader.x;
        let length = 0;
        // Find the null terminator
        while (uint8[x + length] !== 0) {
            length++;
        }
        if (length === 0) {
            reader.x++; // Skip the null byte
            return '';
        }
        const str = reader.utf8(length);
        reader.x++; // Skip the null terminator
        return str;
    }
    readString() {
        const reader = this.reader;
        const length = reader.view.getInt32(reader.x, true); // true = little-endian
        reader.x += 4;
        if (length <= 0) {
            throw new Error('Invalid string length');
        }
        const str = reader.utf8(length - 1); // Length includes null terminator
        reader.x++; // Skip null terminator
        return str;
    }
    readElementValue(type) {
        const reader = this.reader;
        switch (type) {
            case 0x01: {
                // double - 64-bit binary floating point
                const doubleVal = reader.view.getFloat64(reader.x, true);
                reader.x += 8;
                return doubleVal;
            }
            case 0x02: // string - UTF-8 string
                return this.readString();
            case 0x03: // document - Embedded document
                return this.readDocument();
            case 0x04: // array - Array
                return this.readArray();
            case 0x05: // binary - Binary data
                return this.readBinary();
            case 0x06: // undefined (deprecated)
                return undefined;
            case 0x07: // ObjectId
                return this.readObjectId();
            case 0x08: // boolean
                return reader.u8() === 1;
            case 0x09: {
                // UTC datetime
                const dateVal = reader.view.getBigInt64(reader.x, true);
                reader.x += 8;
                return new Date(Number(dateVal));
            }
            case 0x0a: // null
                return null;
            case 0x0b: // regex
                return this.readRegex();
            case 0x0c: // DBPointer (deprecated)
                return this.readDbPointer();
            case 0x0d: // JavaScript code
                return new values_1.BsonJavascriptCode(this.readString());
            case 0x0e: // Symbol (deprecated)
                return Symbol(this.readString());
            case 0x0f: // JavaScript code with scope (deprecated)
                return this.readCodeWithScope();
            case 0x10: {
                // 32-bit integer
                const int32Val = reader.view.getInt32(reader.x, true);
                reader.x += 4;
                return int32Val;
            }
            case 0x11: // Timestamp
                return this.readTimestamp();
            case 0x12: {
                // 64-bit integer
                const int64Val = reader.view.getBigInt64(reader.x, true);
                reader.x += 8;
                return Number(int64Val);
            }
            case 0x13: // 128-bit decimal floating point
                return this.readDecimal128();
            case 0xff: // Min key
                return new values_1.BsonMinKey();
            case 0x7f: // Max key
                return new values_1.BsonMaxKey();
            default:
                throw new Error(`Unsupported BSON type: 0x${type.toString(16)}`);
        }
    }
    readArray() {
        const doc = this.readDocument();
        const keys = Object.keys(doc).sort((a, b) => parseInt(a, 10) - parseInt(b, 10));
        return keys.map((key) => doc[key]);
    }
    readBinary() {
        const reader = this.reader;
        const length = reader.view.getInt32(reader.x, true);
        reader.x += 4;
        const subtype = reader.u8();
        const data = reader.buf(length);
        // For generic binary subtype, return Uint8Array for compatibility
        if (subtype === 0) {
            return data;
        }
        return new values_1.BsonBinary(subtype, data);
    }
    readObjectId() {
        const reader = this.reader;
        const uint8 = reader.uint8;
        const x = reader.x;
        // Timestamp (4 bytes, big-endian)
        const timestamp = (uint8[x] << 24) | (uint8[x + 1] << 16) | (uint8[x + 2] << 8) | uint8[x + 3];
        // Process ID (5 bytes) - first 4 bytes are little-endian, then 1 high byte
        const processLo = uint8[x + 4] | (uint8[x + 5] << 8) | (uint8[x + 6] << 16) | (uint8[x + 7] << 24);
        const processHi = uint8[x + 8];
        // Convert to unsigned 32-bit first, then combine with high byte
        const processLoUnsigned = processLo >>> 0; // Convert to unsigned
        const process = processLoUnsigned + processHi * 0x100000000;
        // Counter (3 bytes, big-endian)
        const counter = (uint8[x + 9] << 16) | (uint8[x + 10] << 8) | uint8[x + 11];
        reader.x += 12;
        return new values_1.BsonObjectId(timestamp, process, counter);
    }
    readRegex() {
        const pattern = this.readCString();
        const flags = this.readCString();
        return new RegExp(pattern, flags);
    }
    readDbPointer() {
        const name = this.readString();
        const id = this.readObjectId();
        return new values_1.BsonDbPointer(name, id);
    }
    readCodeWithScope() {
        const reader = this.reader;
        const _totalLength = reader.view.getInt32(reader.x, true);
        reader.x += 4;
        const code = this.readString();
        const scope = this.readDocument();
        return new values_1.BsonJavascriptCodeWithScope(code, scope);
    }
    readTimestamp() {
        const reader = this.reader;
        const increment = reader.view.getInt32(reader.x, true);
        reader.x += 4;
        const timestamp = reader.view.getInt32(reader.x, true);
        reader.x += 4;
        return new values_1.BsonTimestamp(increment, timestamp);
    }
    readDecimal128() {
        const reader = this.reader;
        const data = reader.buf(16);
        return new values_1.BsonDecimal128(data);
    }
}
exports.BsonDecoder = BsonDecoder;
//# sourceMappingURL=BsonDecoder.js.map