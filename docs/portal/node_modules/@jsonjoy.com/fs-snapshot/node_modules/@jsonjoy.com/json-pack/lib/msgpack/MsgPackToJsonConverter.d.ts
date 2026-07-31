import type { json_string } from '@jsonjoy.com/util/lib/json-brand';
/**
 * @category Decoder
 */
export declare class MsgPackToJsonConverter {
    /** @ignore */
    protected uint8: Uint8Array<ArrayBuffer>;
    /** @ignore */
    protected view: DataView<ArrayBuffer>;
    /** @ignore */
    protected x: number;
    reset(uint8: Uint8Array): void;
    /**
     * Converts a MessagePack blob directly to JSON string.
     *
     * @param uint8 Binary data with MessagePack encoded value.
     * @returns JSON string.
     */
    convert<T = unknown>(uint8: Uint8Array): json_string<T>;
    /** @ignore */
    protected val(): string;
    /** @ignore */
    protected str(size: number): string;
    /** @ignore */
    protected obj(size: number): json_string<object>;
    /** @ignore */
    protected key(): json_string<string>;
    /** @ignore */
    protected arr(size: number): json_string<unknown[]>;
    /** @ignore */
    protected bin(size: number): string;
    /** @ignore */
    protected ext(size: number): string;
    /** @ignore */
    protected u8(): number;
    /** @ignore */
    protected u16(): number;
    /** @ignore */
    protected u32(): number;
    /** @ignore */
    protected i8(): number;
    /** @ignore */
    protected i16(): number;
    /** @ignore */
    protected i32(): number;
    /** @ignore */
    protected f32(): number;
    /** @ignore */
    protected f64(): number;
}
