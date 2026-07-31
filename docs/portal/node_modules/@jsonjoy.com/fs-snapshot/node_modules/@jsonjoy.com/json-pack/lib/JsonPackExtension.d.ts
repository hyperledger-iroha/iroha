/**
 * A wrapping for MessagePack extension or CBOR tag value. When encoder
 * encounters {@link JsonPackExtension} it will encode it as a MessagePack
 * extension or CBOR tag. Likewise, the decoder will
 * decode extensions into {@link JsonPackExtension}.
 *
 * @category Value
 */
export declare class JsonPackExtension<T = Uint8Array> {
    readonly tag: number;
    readonly val: T;
    constructor(tag: number, val: T);
}
