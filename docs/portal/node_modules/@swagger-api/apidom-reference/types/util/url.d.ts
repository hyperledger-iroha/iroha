/**
 * SPDX-FileCopyrightText: Copyright (c) 2015 James Messinger
 *
 * SPDX-License-Identifier: MIT
 */
/**
 * @public
 */
export type WindowsPredicate = () => boolean;
/**
 * Returns the protocol of the given URL, or `undefined` if it has no protocol.
 * @public
 */
export declare const getProtocol: (url: string) => string | undefined;
/**
 * Returns true if given URL has protocol.
 * @public
 */
export declare const hasProtocol: (url: string) => boolean;
/**
 * Returns the lower-cased file extension of the given URL,
 * or an empty string if it has no extension.
 * @public
 */
export declare const getExtension: (url: string) => string;
/**
 * Determines whether the given path is a filesystem path.
 * This includes "file://" URLs.
 * @public
 */
export declare const isFileSystemPath: (uri: string) => boolean;
/**
 * Determines whether the given URI is an HTTP(S) URL.
 * @public
 */
export declare const isHttpUrl: (url: string) => boolean;
/**
 * Determines whether the given URI
 * @public
 */
export declare const isURI: (uri: string) => boolean;
/**
 * @public
 */
export interface ToFileSystemPathOptions {
    keepFileProtocol?: boolean;
    isWindows?: WindowsPredicate;
}
/**
 * Converts a URL to a local filesystem path.
 * @public
 */
export declare const toFileSystemPath: (uri: string, options?: ToFileSystemPathOptions) => string;
/**
 * Converts a filesystem path to a properly-encoded URL.
 *
 * This is intended to handle situations where resolver is called
 * with a filesystem path that contains characters which are not allowed in URLs.
 *
 * @example
 * The following filesystem paths would be converted to the following URLs:
 *```
 *    <"!@#$%^&*+=?'>.json              ==>   %3C%22!@%23$%25%5E&*+=%3F\'%3E.json
 *    C:\\My Documents\\File (1).json   ==>   C:/My%20Documents/File%20(1).json
 *    file://Project #42/file.json      ==>   file://Project%20%2342/file.json
 * ```
 * @public
 */
export declare const fromFileSystemPath: (uri: string) => string;
/**
 * Returns the hash (URL fragment), of the given path.
 * If there is no hash, then the root hash ("#") is returned.
 * @public
 */
export declare const getHash: (uri: string) => string;
/**
 * Removes the hash (URL fragment), if any, from the given path.
 * @public
 */
export declare const stripHash: (uri: string) => string;
/**
 * Returns the current working directory (in Node) or the current page URL (in browsers).
 * @public
 */
export declare const cwd: () => string;
/**
 * Resolves a target URI relative to a base URI in a manner similar to that of a Web browser resolving an anchor tag HREF.
 * @public
 */
export declare const resolve: (from: string, to: string) => string;
/**
 * Sanitizes/Encodes URI to it's url encoded form.
 *
 * The functional will compensate with the usecase when
 * already sanitized URI is passed to it,
 * by first unsatizing it and then performing sanitization again.
 * @public
 */
export declare const sanitize: (uri: string) => string;
/**
 * Unsanitizes/Decodes URI to it's url encoded form.
 * This function already assumes that hash part of the URI
 * has been removed prior to transforming it to it's sanitized form.
 * @public
 */
export declare const unsanitize: (uri: string) => string;
