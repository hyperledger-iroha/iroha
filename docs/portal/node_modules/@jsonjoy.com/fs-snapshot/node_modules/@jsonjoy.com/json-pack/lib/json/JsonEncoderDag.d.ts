import { JsonEncoderStable } from './JsonEncoderStable';
/**
 * Base class for implementing DAG-JSON encoders.
 *
 * @see https://ipld.io/specs/codecs/dag-json/spec/
 */
export declare class JsonEncoderDag extends JsonEncoderStable {
    /**
     * Encodes binary data as nested `["/", "bytes"]` object encoded in Base64
     * without padding.
     *
     * Example:
     *
     * ```json
     * {"/":{"bytes":"aGVsbG8gd29ybGQ"}}
     * ```
     *
     * @param buf Binary data to write.
     */
    writeBin(buf: Uint8Array): void;
    writeCid(cid: string): void;
}
