import { CborEncoder } from './CborEncoder';
export declare class CborEncoderStable extends CborEncoder {
    writeObj(obj: Record<string, unknown>): void;
    /** @todo This implementation might be even faster than the default one, verify that. */
    writeStr(str: string): void;
    writeUndef(): void;
}
