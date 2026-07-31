import { ParseResultElement } from '@swagger-api/apidom-core';
/**
 * This class represents a File object with url and data.
 * @public
 */
export interface FileOptions {
    readonly uri: string;
    readonly mediaType?: string;
    readonly data?: Buffer | DataView | ArrayBuffer | string;
    readonly parseResult?: ParseResultElement;
}
/**
 * @public
 */
declare class File {
    uri: string;
    mediaType: string;
    data?: Buffer | DataView | ArrayBuffer | string;
    parseResult?: ParseResultElement;
    constructor({ uri, mediaType, data, parseResult }: FileOptions);
    get extension(): string;
    toString(): string;
}
export default File;
