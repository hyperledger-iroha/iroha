/// <reference types="node" />
import { SAXStream } from 'sax';
import { Readable, Transform, TransformOptions, TransformCallback } from 'stream';
import { IndexItem, ErrorLevel } from './types';
declare type Logger = (level: 'warn' | 'error' | 'info' | 'log', ...message: Parameters<Console['log']>[0]) => void;
export interface XMLToSitemapIndexItemStreamOptions extends TransformOptions {
    level?: ErrorLevel;
    logger?: Logger | false;
}
/**
 * Takes a stream of xml and transforms it into a stream of IndexItems
 * Use this to parse existing sitemap indices into config options compatible with this library
 */
export declare class XMLToSitemapIndexStream extends Transform {
    level: ErrorLevel;
    logger: Logger;
    saxStream: SAXStream;
    constructor(opts?: XMLToSitemapIndexItemStreamOptions);
    _transform(data: string, encoding: string, callback: TransformCallback): void;
}
export declare function parseSitemapIndex(xml: Readable, maxEntries?: number): Promise<IndexItem[]>;
export interface IndexObjectStreamToJSONOptions extends TransformOptions {
    lineSeparated: boolean;
}
/**
 * A Transform that converts a stream of objects into a JSON Array or a line
 * separated stringified JSON
 * @param [lineSeparated=false] whether to separate entries by a new line or comma
 */
export declare class IndexObjectStreamToJSON extends Transform {
    lineSeparated: boolean;
    firstWritten: boolean;
    constructor(opts?: IndexObjectStreamToJSONOptions);
    _transform(chunk: IndexItem, encoding: string, cb: TransformCallback): void;
    _flush(cb: TransformCallback): void;
}
export {};
