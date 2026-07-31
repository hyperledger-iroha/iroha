import { StreamingReader } from '@jsonjoy.com/buffers/lib/StreamingReader';
import { RespDecoder } from './RespDecoder';
/**
 * Streaming decoder for RESP protocol. Can be used to decode data from
 * a stream where messages are arbitrary split into chunks.
 *
 * Example:
 *
 * ```ts
 * const decoder = new RespStreamingDecoder();
 *
 * decoder.push(new Uint8Array([43, 49, 13, 10]));
 *
 * let val;
 * while ((val = decoder.read()) !== undefined) {
 *   console.log(val);
 * }
 * ```
 */
export declare class RespStreamingDecoder {
    protected readonly reader: StreamingReader;
    protected readonly decoder: RespDecoder<StreamingReader>;
    /**
     * When set to true, the decoder will attempt to decode RESP Bulk strings
     * (which are binary strings, i.e. Uint8Array) as UTF-8 strings. If the
     * string is not valid UTF-8, it will be returned as a Uint8Array.
     */
    get tryUtf8(): boolean;
    set tryUtf8(value: boolean);
    /**
     * Add a chunk of data to be decoded.
     * @param uint8 `Uint8Array` chunk of data to be decoded.
     */
    push(uint8: Uint8Array): void;
    /**
     * Decode one value from the stream. If `undefined` is returned, then
     * there is not enough data to decode or the stream is finished.
     *
     * There could be multiple values in the stream, so this method should be
     * called in a loop until `undefined` is returned.
     *
     * @return Decoded value or `undefined` if there is not enough data to decode.
     */
    read(): unknown | undefined;
    /**
     * Decode only one RESP command from the stream, if the value is not a
     * command, an error will be thrown.
     *
     * @returns Redis command and its arguments or `undefined` if there is
     * not enough data to decode.
     */
    readCmd(): [cmd: string, ...args: Uint8Array[]] | undefined;
    /**
     * Skips one value from the stream. If `undefined` is returned, then
     * there is not enough data to skip or the stream is finished.
     * @returns `null` if a value was skipped, `undefined` if there is not
     * enough data to skip.
     */
    skip(): null | undefined;
}
