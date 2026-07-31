import { Node } from './Node';
import { Link } from './Link';
import { File } from './File';
import { Buffer } from '@jsonjoy.com/fs-node-builtins/lib/internal/buffer';
import { type IProcess } from './process';
import { DirectoryJSON, NestedDirectoryJSON } from './json';
import type { PathLike } from '@jsonjoy.com/fs-node-utils/lib/types/misc';
import { TFileId, StatError } from './types';
import { Result } from './result';
import { FanOut } from 'thingies/lib/fanout';
import { FsEvent } from './watch/FsEvent';
/**
 * Represents a filesystem superblock, which is the root of a virtual
 * filesystem in Linux.
 * @see https://lxr.linux.no/linux+v3.11.2/include/linux/fs.h#L1242
 */
export declare class Superblock {
    static fromJSON(json: DirectoryJSON, cwd?: string, opts?: {
        process?: IProcess;
    }): Superblock;
    static fromNestedJSON(json: NestedDirectoryJSON, cwd?: string, opts?: {
        process?: IProcess;
    }): Superblock;
    /**
     * Global file descriptor counter. UNIX file descriptors start from 0 and go sequentially
     * up, so here, in order not to conflict with them, we choose some big number and descrease
     * the file descriptor of every new opened file.
     * @type {number}
     * @todo This should not be static, right?
     */
    static fd: number;
    root: Link;
    ino: number;
    inodes: {
        [ino: number]: Node;
    };
    releasedInos: number[];
    fds: {
        [fd: number]: File;
    };
    releasedFds: number[];
    maxFiles: number;
    openFiles: number;
    /** The `process`-like object used by this filesystem instance. */
    readonly process: IProcess;
    constructor(opts?: {
        process?: IProcess;
    });
    /** Fan-out of file system change events. Multiple consumers may subscribe. */
    readonly changes: FanOut<FsEvent>;
    protected emit(change: FsEvent): void;
    createLink(): Link;
    createLink(parent: Link, name: string, isDirectory?: boolean, mode?: number): Link;
    deleteLink(link: Link): boolean;
    private _emitDeleteRecursive;
    private newInoNumber;
    private newFdNumber;
    createNode(mode: number): Node;
    deleteNode(node: Node): void;
    walk(steps: string[], resolveSymlinks: boolean, checkExistence: boolean, checkAccess: boolean, funcName?: string): Result<Link | null, StatError>;
    walk(filename: string, resolveSymlinks: boolean, checkExistence: boolean, checkAccess: boolean, funcName?: string): Result<Link | null, StatError>;
    walk(link: Link, resolveSymlinks: boolean, checkExistence: boolean, checkAccess: boolean, funcName?: string): Result<Link | null, StatError>;
    walk(stepsOrFilenameOrLink: string[] | string | Link, resolveSymlinks: boolean, checkExistence: boolean, checkAccess: boolean, funcName?: string): Result<Link | null, StatError>;
    getLink(steps: string[]): Link | null;
    getLinkOrThrow(filename: string, funcName?: string): Link;
    getResolvedLink(filenameOrSteps: string | string[]): Link | null;
    /**
     * Just like `getLinkOrThrow`, but also dereference/resolves symbolic links.
     */
    getResolvedLinkOrThrow(filename: string, funcName?: string): Link;
    getResolvedLinkResult(filename: string, funcName?: string): Result<Link, StatError>;
    resolveSymlinks(link: Link): Link | null;
    /**
     * Just like `getLinkOrThrow`, but also verifies that the link is a directory.
     */
    getLinkAsDirOrThrow(filename: string, funcName?: string): Link;
    getLinkParent(steps: string[]): Link | null;
    getLinkParentAsDirOrThrow(filenameOrSteps: string | string[], funcName?: string): Link;
    getFileByFd(fd: number): File;
    getFileByFdOrThrow(fd: number, funcName?: string): File;
    _toJSON(link?: Link, json?: {}, path?: string, asBuffer?: boolean): DirectoryJSON<string | null>;
    toJSON(paths?: PathLike | PathLike[], json?: {}, isRelative?: boolean, asBuffer?: boolean): DirectoryJSON<string | null>;
    fromJSON(json: DirectoryJSON, cwd?: string): void;
    fromNestedJSON(json: NestedDirectoryJSON, cwd?: string): void;
    reset(): void;
    mountSync(mountpoint: string, json: DirectoryJSON): void;
    openLink(link: Link, flagsNum: number, resolveSymlinks?: boolean): File;
    protected openFile(filename: string, flagsNum: number, modeNum: number | undefined, resolveSymlinks?: boolean): File;
    readonly open: (filename: string, flagsNum: number, modeNum: number, resolveSymlinks?: boolean) => number;
    readonly writeFile: (id: TFileId, buf: Buffer, flagsNum: number, modeNum: number) => void;
    readonly read: (fd: number, buffer: Buffer | ArrayBufferView | DataView, offset: number, length: number, position: number | null) => number;
    readonly readv: (fd: number, buffers: ArrayBufferView[], position: number | null) => number;
    readonly link: (filename1: string, filename2: string) => void;
    readonly unlink: (filename: string) => void;
    readonly symlink: (targetFilename: string, pathFilename: string) => Link;
    readonly rename: (oldPathFilename: string, newPathFilename: string) => void;
    readonly mkdir: (filename: string, modeNum: number) => void;
    /**
     * Creates directory tree recursively.
     */
    readonly mkdirp: (filename: string, modeNum: number) => string | undefined;
    readonly rmdir: (filename: string, recursive?: boolean) => void;
    readonly rm: (filename: string, force?: boolean, recursive?: boolean) => void;
    protected closeFile(file: File): void;
    readonly close: (fd: number) => void;
    write(fd: number, buf: Buffer, offset?: number, length?: number, position?: number | null): number;
    readonly ftruncate: (fd: number, len?: number) => void;
    readonly fchmod: (fd: number, modeNum: number) => void;
    readonly chmod: (filename: string, modeNum: number) => void;
    readonly lchmod: (filename: string, modeNum: number) => void;
    readonly fchown: (fd: number, uid: number, gid: number) => void;
    readonly chown: (filename: string, uid: number, gid: number) => void;
    readonly lchown: (filename: string, uid: number, gid: number) => void;
    readonly futimes: (fd: number, atime: number, mtime: number) => void;
    readonly utimes: (filename: string, atime: number, mtime: number, followSymlinks?: boolean) => void;
}
