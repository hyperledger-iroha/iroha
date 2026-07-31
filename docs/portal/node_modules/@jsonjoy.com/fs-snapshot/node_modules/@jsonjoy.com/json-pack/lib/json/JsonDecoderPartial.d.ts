import { JsonDecoder } from './JsonDecoder';
import type { PackValue } from '../types';
export declare class DecodeFinishError extends Error {
    readonly value: unknown;
    constructor(value: unknown);
}
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
export declare class JsonDecoderPartial extends JsonDecoder {
    readAny(): unknown;
    readArr(): unknown[];
    readObj(): PackValue | Record<string, unknown> | unknown;
}
