"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.JsonDecoderPartial = exports.DecodeFinishError = void 0;
const JsonDecoder_1 = require("./JsonDecoder");
class DecodeFinishError extends Error {
    constructor(value) {
        super('DECODE_FINISH');
        this.value = value;
    }
}
exports.DecodeFinishError = DecodeFinishError;
/**
 * This class parses JSON which is mostly correct but not necessarily complete
 * or with missing parts. It can be used to parse JSON that is being streamed
 * in chunks or JSON output of an LLM model.
 *
 * If the end of a nested JSON value (array, object) is missing, this parser
 * will return the initial correct part for that value, which it was able to
 * parse, until the point where the JSON is no longer valid.
 *
 * Examples:
 *
 * ```js
 * // Missing closing brace
 * decoder.readAny('[1, 2, 3'); // [1, 2, 3]
 *
 * // Trailing comma and missing closing brace
 * decoder.readAny('[1, 2, '); // [1, 2]
 *
 * // Corrupt second element and missing closing brace
 * decoder.readAny('{"foo": 1, "bar":'); // {"foo": 1}
 * ```
 */
class JsonDecoderPartial extends JsonDecoder_1.JsonDecoder {
    readAny() {
        try {
            return super.readAny();
        }
        catch (error) {
            if (error instanceof DecodeFinishError)
                return error.value;
            throw error;
        }
    }
    readArr() {
        const reader = this.reader;
        if (reader.u8() !== 0x5b /* [ */)
            throw new Error('Invalid JSON');
        const arr = [];
        const uint8 = reader.uint8;
        let first = true;
        while (true) {
            this.skipWhitespace();
            const char = uint8[reader.x];
            if (char === 0x5d /* ] */)
                return reader.x++, arr;
            if (char === 0x2c /* , */)
                reader.x++;
            else if (!first)
                return arr;
            this.skipWhitespace();
            try {
                arr.push(this.readAny());
            }
            catch (error) {
                if (error instanceof DecodeFinishError)
                    return arr.push(error.value), arr;
                if (error instanceof Error && error.message === 'Invalid JSON')
                    throw new DecodeFinishError(arr);
                throw error;
            }
            first = false;
        }
    }
    readObj() {
        const reader = this.reader;
        if (reader.u8() !== 0x7b /* { */)
            throw new Error('Invalid JSON');
        const obj = {};
        const uint8 = reader.uint8;
        while (true) {
            this.skipWhitespace();
            let char = uint8[reader.x];
            if (char === 0x7d /* } */)
                return reader.x++, obj;
            if (char === 0x2c /* , */) {
                reader.x++;
                continue;
            }
            try {
                char = uint8[reader.x++];
                if (char !== 0x22 /* " */)
                    throw new Error('Invalid JSON');
                const key = (0, JsonDecoder_1.readKey)(reader);
                if (key === '__proto__')
                    throw new Error('Invalid JSON');
                this.skipWhitespace();
                if (reader.u8() !== 0x3a /* : */)
                    throw new Error('Invalid JSON');
                this.skipWhitespace();
                try {
                    obj[key] = this.readAny();
                }
                catch (error) {
                    if (error instanceof DecodeFinishError) {
                        obj[key] = error.value;
                        return obj;
                    }
                    throw error;
                }
            }
            catch (error) {
                if (error instanceof DecodeFinishError)
                    return obj;
                if (error instanceof Error && error.message === 'Invalid JSON')
                    throw new DecodeFinishError(obj);
                throw error;
            }
        }
    }
}
exports.JsonDecoderPartial = JsonDecoderPartial;
//# sourceMappingURL=JsonDecoderPartial.js.map