import type { NodeFsaContext } from './types';
export { basename } from '@jsonjoy.com/fs-node-utils';
/**
 * Creates a new {@link NodeFsaContext}.
 */
export declare const ctx: (partial?: Partial<NodeFsaContext>) => NodeFsaContext;
export declare const assertName: (name: string, method: string, klass: string) => void;
export declare const assertCanWrite: (mode: "read" | "readwrite") => void;
export declare const newNotFoundError: () => DOMException;
export declare const newTypeMismatchError: () => DOMException;
export declare const newNotAllowedError: () => DOMException;
export declare const newNoModificationAllowedError: () => DOMException;
