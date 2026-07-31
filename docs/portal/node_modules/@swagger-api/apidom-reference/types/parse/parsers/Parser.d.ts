import { ParseResultElement } from '@swagger-api/apidom-core';
import File from '../../File.ts';
/**
 * @public
 */
export interface ParserOptions {
    readonly name: string;
    readonly allowEmpty?: boolean;
    readonly sourceMap?: boolean;
    readonly fileExtensions?: string[];
    readonly mediaTypes?: string[];
}
/**
 * @public
 */
declare abstract class Parser {
    readonly name: string;
    /**
     * Whether to allow "empty" files. This includes zero-byte files.
     */
    allowEmpty: boolean;
    /**
     * Whether to generate source map during parsing.
     */
    sourceMap: boolean;
    /**
     * List of supported file extensions.
     */
    fileExtensions: string[];
    /**
     * List of supported media types.
     */
    mediaTypes: string[];
    constructor({ name, allowEmpty, sourceMap, fileExtensions, mediaTypes, }: ParserOptions);
    abstract canParse(file: File): boolean | Promise<boolean>;
    abstract parse(file: File): ParseResultElement | Promise<ParseResultElement>;
}
export default Parser;
