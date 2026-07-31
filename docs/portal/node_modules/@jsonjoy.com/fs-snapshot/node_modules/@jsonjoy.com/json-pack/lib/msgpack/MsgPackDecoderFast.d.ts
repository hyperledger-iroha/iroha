import { Reader } from '@jsonjoy.com/buffers/lib/Reader';
import { JsonPackExtension } from '../JsonPackExtension';
import type { BinaryJsonDecoder, PackValue } from '../types';
import type { CachedUtf8Decoder } from '@jsonjoy.com/buffers/lib/utf8/CachedUtf8Decoder';
/**
 * @category Decoder
 */
export declare class MsgPackDecoderFast<R extends Reader> implements BinaryJsonDecoder {
    reader: R;
    protected readonly keyDecoder: CachedUtf8Decoder;
    constructor(reader?: R, keyDecoder?: CachedUtf8Decoder);
    /** @deprecated */
    decode(uint8: Uint8Array): unknown;
    read(uint8: Uint8Array): PackValue;
    val(): unknown;
    readAny(): unknown;
    str(): unknown;
    /** @ignore */
    protected obj(size: number): object;
    /** @ignore */
    protected key(): string;
    /** @ignore */
    protected arr(size: number): unknown[];
    /** @ignore */
    protected ext(size: number): JsonPackExtension;
    protected back(bytes: number): void;
}
