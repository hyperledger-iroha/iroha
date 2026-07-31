"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.RespDecoder = void 0;
const Reader_1 = require("@jsonjoy.com/buffers/lib/Reader");
const extensions_1 = require("./extensions");
const isUtf8_1 = require("@jsonjoy.com/buffers/lib/utf8/isUtf8");
class RespDecoder {
    constructor(reader = new Reader_1.Reader()) {
        this.reader = reader;
        /**
         * When set to true, the decoder will attempt to decode RESP Bulk strings
         * (which are binary strings, i.e. Uint8Array) as UTF-8 strings. If the
         * string is not valid UTF-8, it will be returned as a Uint8Array.
         *
         * You can toggle this setting at any time, before each call to `decode()`
         * or `read()`, or other methods.
         */
        this.tryUtf8 = false;
    }
    read(uint8) {
        this.reader.reset(uint8);
        return this.readAny();
    }
    /** @deprecated */
    decode(uint8) {
        this.reader.reset(uint8);
        return this.readAny();
    }
    // -------------------------------------------------------- Any value reading
    val() {
        return this.readAny();
    }
    readAny() {
        const reader = this.reader;
        const type = reader.u8();
        switch (type) {
            case 58 /* RESP.INT */:
                return this.readInt();
            case 44 /* RESP.FLOAT */:
                return this.readFloat();
            case 43 /* RESP.STR_SIMPLE */:
                return this.readStrSimple();
            case 36 /* RESP.STR_BULK */:
                return this.readStrBulk();
            case 35 /* RESP.BOOL */:
                return this.readBool();
            case 95 /* RESP.NULL */:
                return reader.skip(2), null;
            case 37 /* RESP.OBJ */:
                return this.readObj();
            case 42 /* RESP.ARR */:
                return this.readArr();
            case 61 /* RESP.STR_VERBATIM */:
                return this.readStrVerbatim();
            case 62 /* RESP.PUSH */:
                return new extensions_1.RespPush(this.readArr() || []);
            case 40 /* RESP.BIG */:
                return this.readBigint();
            case 126 /* RESP.SET */:
                return this.readSet();
            case 45 /* RESP.ERR_SIMPLE */:
                return this.readErrSimple();
            case 33 /* RESP.ERR_BULK */:
                return this.readErrBulk();
            case 124 /* RESP.ATTR */:
                return new extensions_1.RespAttributes(this.readObj());
        }
        throw new Error('UNKNOWN_TYPE');
    }
    readLength() {
        const reader = this.reader;
        let number = 0;
        while (true) {
            const c = reader.u8();
            if (c === 13 /* RESP.R */)
                return reader.skip(1), number;
            number = number * 10 + (c - 48);
        }
    }
    readCmd() {
        const reader = this.reader;
        const type = reader.u8();
        if (type !== 42 /* RESP.ARR */)
            throw new Error('INVALID_COMMAND');
        const c = reader.peak();
        if (c === 45 /* RESP.MINUS */)
            throw new Error('INVALID_COMMAND');
        const length = this.readLength();
        if (length === 0)
            throw new Error('INVALID_COMMAND');
        const cmd = this.readAsciiAsStrBulk().toUpperCase();
        const args = [cmd];
        this.tryUtf8 = false;
        for (let i = 1; i < length; i++) {
            const type = reader.u8();
            if (type !== 36 /* RESP.STR_BULK */)
                throw new Error('INVALID_COMMAND');
            args.push(this.readStrBulk());
        }
        return args;
    }
    // ---------------------------------------------------------- Boolean reading
    readBool() {
        const reader = this.reader;
        const c = reader.u8();
        reader.skip(2); // Skip "\r\n".
        return c === 116; // t
    }
    // ----------------------------------------------------------- Number reading
    readInt() {
        const reader = this.reader;
        let negative = false;
        let c = reader.u8();
        let number = 0;
        if (c === 45 /* RESP.MINUS */) {
            negative = true;
        }
        else if (c !== 43 /* RESP.PLUS */)
            number = c - 48;
        while (true) {
            c = reader.u8();
            if (c === 13 /* RESP.R */) {
                reader.skip(1); // Skip "\n".
                return negative ? -number : number;
            }
            number = number * 10 + (c - 48);
        }
    }
    readFloat() {
        const reader = this.reader;
        const x = reader.x;
        while (true) {
            const c = reader.u8();
            if (c !== 13 /* RESP.R */)
                continue;
            const length = reader.x - x - 1;
            reader.x = x;
            const str = reader.ascii(length);
            switch (length) {
                case 3:
                    switch (str) {
                        case 'inf':
                            return reader.skip(2), Infinity;
                        case 'nan':
                            return reader.skip(2), NaN;
                    }
                    break;
                case 4:
                    if (str === '-inf') {
                        return reader.skip(2), -Infinity;
                    }
                    break;
            }
            reader.skip(2); // Skip "\n".
            return Number(str);
        }
    }
    readBigint() {
        const reader = this.reader;
        const x = reader.x;
        while (true) {
            const c = reader.u8();
            if (c !== 13 /* RESP.R */)
                continue;
            const length = reader.x - x;
            reader.x = x;
            const str = reader.ascii(length);
            reader.skip(1); // Skip "\n".
            return BigInt(str);
        }
    }
    // ----------------------------------------------------------- String reading
    readStrSimple() {
        const reader = this.reader;
        const x = reader.x;
        while (true) {
            const c = reader.u8();
            if (c !== 13 /* RESP.R */)
                continue;
            const size = reader.x - x - 1;
            reader.x = x;
            const str = reader.utf8(size);
            reader.skip(2); // Skip "\r\n".
            return str;
        }
    }
    readStrBulk() {
        const reader = this.reader;
        if (reader.peak() === 45 /* RESP.MINUS */) {
            reader.skip(4); // Skip "-1\r\n".
            return null;
        }
        const length = this.readLength();
        let res;
        if (this.tryUtf8 && (0, isUtf8_1.isUtf8)(reader.uint8, reader.x, length))
            res = reader.utf8(length);
        else
            res = reader.buf(length);
        reader.skip(2); // Skip "\r\n".
        return res;
    }
    readAsciiAsStrBulk() {
        const reader = this.reader;
        reader.skip(1); // Skip "$".
        const length = this.readLength();
        const buf = reader.ascii(length);
        reader.skip(2); // Skip "\r\n".
        return buf;
    }
    readStrVerbatim() {
        const reader = this.reader;
        const length = this.readLength();
        const u32 = reader.u32();
        const isTxt = u32 === 1954051130; // "txt:"
        if (isTxt) {
            const str = reader.utf8(length - 4);
            reader.skip(2); // Skip "\r\n".
            return str;
        }
        const buf = reader.buf(length - 4);
        reader.skip(2); // Skip "\r\n".
        return buf;
    }
    // ------------------------------------------------------------ Error reading
    readErrSimple() {
        const reader = this.reader;
        const x = reader.x;
        while (true) {
            const c = reader.u8();
            if (c !== 13 /* RESP.R */)
                continue;
            const size = reader.x - x - 1;
            reader.x = x;
            const str = reader.utf8(size);
            reader.skip(2); // Skip "\r\n".
            return new Error(str);
        }
    }
    readErrBulk() {
        const reader = this.reader;
        const length = this.readLength();
        const message = reader.utf8(length);
        reader.skip(2); // Skip "\r\n".
        return new Error(message);
    }
    // ------------------------------------------------------------ Array reading
    readArr() {
        const reader = this.reader;
        const c = reader.peak();
        if (c === 45 /* RESP.MINUS */) {
            reader.skip(4); // Skip "-1\r\n".
            return null;
        }
        const length = this.readLength();
        const arr = [];
        for (let i = 0; i < length; i++)
            arr.push(this.readAny());
        return arr;
    }
    readSet() {
        const length = this.readLength();
        const set = new Set();
        for (let i = 0; i < length; i++)
            set.add(this.readAny());
        return set;
    }
    // ----------------------------------------------------------- Object reading
    readObj() {
        const length = this.readLength();
        const obj = {};
        for (let i = 0; i < length; i++) {
            const key = this.readAny() + '';
            obj[key] = this.readAny();
        }
        return obj;
    }
    // ----------------------------------------------------------------- Skipping
    skipN(n) {
        for (let i = 0; i < n; i++)
            this.skipAny();
    }
    skipAny() {
        const reader = this.reader;
        const type = reader.u8();
        switch (type) {
            case 58 /* RESP.INT */:
                return this.skipInt();
            case 44 /* RESP.FLOAT */:
                return this.skipFloat();
            case 43 /* RESP.STR_SIMPLE */:
                return this.skipStrSimple();
            case 36 /* RESP.STR_BULK */:
                return this.skipStrBulk();
            case 35 /* RESP.BOOL */:
                return this.skipBool();
            case 95 /* RESP.NULL */:
                return reader.skip(2);
            case 37 /* RESP.OBJ */:
                return this.skipObj();
            case 42 /* RESP.ARR */:
                return this.skipArr();
            case 61 /* RESP.STR_VERBATIM */:
                return this.skipStrVerbatim();
            case 62 /* RESP.PUSH */:
                return this.skipArr();
            case 40 /* RESP.BIG */:
                return this.skipBigint();
            case 126 /* RESP.SET */:
                return this.skipSet();
            case 45 /* RESP.ERR_SIMPLE */:
                return this.skipErrSimple();
            case 33 /* RESP.ERR_BULK */:
                return this.skipErrBulk();
            case 124 /* RESP.ATTR */:
                return this.skipObj();
        }
        throw new Error('UNKNOWN_TYPE');
    }
    skipBool() {
        this.reader.skip(3);
    }
    skipInt() {
        const reader = this.reader;
        while (true) {
            if (reader.u8() !== 13 /* RESP.R */)
                continue;
            reader.skip(1); // Skip "\n".
            return;
        }
    }
    skipFloat() {
        const reader = this.reader;
        while (true) {
            if (reader.u8() !== 13 /* RESP.R */)
                continue;
            reader.skip(1); // Skip "\n".
            return;
        }
    }
    skipBigint() {
        const reader = this.reader;
        while (true) {
            if (reader.u8() !== 13 /* RESP.R */)
                continue;
            reader.skip(1); // Skip "\n".
            return;
        }
    }
    skipStrSimple() {
        const reader = this.reader;
        while (true) {
            if (reader.u8() !== 13 /* RESP.R */)
                continue;
            reader.skip(1); // Skip "\n".
            return;
        }
    }
    skipStrBulk() {
        const reader = this.reader;
        if (reader.peak() === 45 /* RESP.MINUS */) {
            reader.skip(4); // Skip "-1\r\n".
            return;
        }
        reader.skip(this.readLength() + 2); // Skip "\r\n".
    }
    skipStrVerbatim() {
        const length = this.readLength();
        this.reader.skip(length + 2); // Skip "\r\n".
    }
    skipErrSimple() {
        const reader = this.reader;
        while (true) {
            if (reader.u8() !== 13 /* RESP.R */)
                continue;
            reader.skip(1); // Skip "\n".
            return;
        }
    }
    skipErrBulk() {
        const length = this.readLength();
        this.reader.skip(length + 2); // Skip "\r\n".
    }
    skipArr() {
        const reader = this.reader;
        const c = reader.peak();
        if (c === 45 /* RESP.MINUS */) {
            reader.skip(4); // Skip "-1\r\n".
            return;
        }
        const length = this.readLength();
        for (let i = 0; i < length; i++)
            this.skipAny();
    }
    skipSet() {
        const length = this.readLength();
        for (let i = 0; i < length; i++)
            this.skipAny();
    }
    skipObj() {
        const length = this.readLength();
        for (let i = 0; i < length; i++) {
            this.skipAny();
            this.skipAny();
        }
    }
}
exports.RespDecoder = RespDecoder;
//# sourceMappingURL=RespDecoder.js.map