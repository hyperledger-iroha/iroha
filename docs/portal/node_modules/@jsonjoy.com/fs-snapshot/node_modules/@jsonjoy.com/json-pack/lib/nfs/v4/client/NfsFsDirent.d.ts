import type * as misc from 'memfs/lib/node/types/misc';
import { Nfsv4FType } from '../constants';
/**
 * Implements Node.js-like Dirent interface for NFS v4 directory entries.
 */
export declare class NfsFsDirent implements misc.IDirent {
    name: string;
    private type;
    constructor(name: string, type: Nfsv4FType);
    isDirectory(): boolean;
    isFile(): boolean;
    isBlockDevice(): boolean;
    isCharacterDevice(): boolean;
    isSymbolicLink(): boolean;
    isFIFO(): boolean;
    isSocket(): boolean;
}
