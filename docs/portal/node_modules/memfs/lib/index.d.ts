import { Stats, Dirent, Volume, StatWatcher, FSWatcher } from '@jsonjoy.com/fs-node';
import type { IWriteStream } from '@jsonjoy.com/fs-node';
import { DirectoryJSON, NestedDirectoryJSON, type IProcess } from '@jsonjoy.com/fs-core';
import { constants } from '@jsonjoy.com/fs-node-utils';
import type { FsPromisesApi } from '@jsonjoy.com/fs-node-utils';
import type * as misc from '@jsonjoy.com/fs-node-utils/lib/types/misc';
export { DirectoryJSON, NestedDirectoryJSON, Volume };
export type { IProcess };
export declare const vol: Volume;
export interface IFs extends Volume {
    constants: typeof constants;
    Stats: new (...args: any[]) => Stats;
    Dirent: new (...args: any[]) => Dirent;
    StatWatcher: new () => StatWatcher;
    FSWatcher: new () => FSWatcher;
    ReadStream: new (...args: any[]) => misc.IReadStream;
    WriteStream: new (...args: any[]) => IWriteStream;
    promises: FsPromisesApi;
    _toUnixTimestamp: any;
}
export declare function createFsFromVolume(vol: Volume): IFs;
export declare const fs: IFs;
/** Options for creating a memfs instance. */
export interface MemfsOptions {
    /** Custom working directory for resolving relative paths. Defaults to `'/'`. */
    cwd?: string;
    /** Custom `process`-like object for controlling platform, uid, gid, and cwd behavior. */
    process?: IProcess;
}
/**
 * Creates a new file system instance.
 *
 * @param json File system structure expressed as a JSON object.
 *        Use `null` for empty directories and empty string for empty files.
 * @param cwdOrOpts Current working directory (string) or options object.
 *        The JSON structure will be created relative to the cwd path.
 * @returns A `memfs` file system instance, which is a drop-in replacement for
 *          the `fs` module.
 */
export declare const memfs: (json?: NestedDirectoryJSON, cwdOrOpts?: string | MemfsOptions) => {
    fs: IFs;
    vol: Volume;
};
export type IFsWithVolume = IFs & {
    __vol: Volume;
};
