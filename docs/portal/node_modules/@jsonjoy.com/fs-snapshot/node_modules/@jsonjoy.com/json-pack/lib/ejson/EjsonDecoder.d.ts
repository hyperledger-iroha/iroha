import { JsonDecoder } from '../json/JsonDecoder';
export interface EjsonDecoderOptions {
    /** Whether to parse legacy Extended JSON formats */
    legacy?: boolean;
}
export declare class EjsonDecoder extends JsonDecoder {
    private options;
    constructor(options?: EjsonDecoderOptions);
    /**
     * Decode from string (for backward compatibility).
     * This method maintains the previous API but uses the binary decoder internally.
     */
    decodeFromString(json: string): unknown;
    readAny(): unknown;
    readArr(): unknown[];
    readObjWithEjsonSupport(): unknown;
    private readValue;
    private readRawObj;
    private transformEjsonObject;
    private parseObjectId;
    private base64ToUint8Array;
    private isValidUuid;
    private uuidToBytes;
}
