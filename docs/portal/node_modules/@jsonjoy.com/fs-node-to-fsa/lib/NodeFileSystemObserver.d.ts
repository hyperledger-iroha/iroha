import type { IFileSystemChangeRecord, IFileSystemDirectoryHandle, IFileSystemFileHandle, IFileSystemHandle, IFileSystemObserver, IFileSystemObserverObserveOptions, IFileSystemSyncAccessHandle } from '@jsonjoy.com/fs-fsa';
import type { FsCallbackApi } from '@jsonjoy.com/fs-node-utils';
import type * as misc from '@jsonjoy.com/fs-node-utils/lib/types/misc';
import type { NodeFsaContext, NodeFsaFs } from './types';
export type NodeFsaWatchFs = NodeFsaFs & Pick<FsCallbackApi, 'watch'>;
/**
 * A `FileSystemObserver` implementation backed by the underlying Node.js-like
 * `fs.watch`. This is a best-effort profile, per the File System Observer
 * proposal's allowance for local file systems: `rename` events are classified
 * into `"appeared"`/`"disappeared"` records by stat-ing the path, no
 * `"moved"` records are ever produced (pairing renames is unreliable — same
 * as Chrome on Windows), and a backend watcher error surfaces as a terminal
 * `"errored"` record for that observation.
 *
 * @see https://developer.mozilla.org/en-US/docs/Web/API/FileSystemObserver
 */
export declare class NodeFileSystemObserver implements IFileSystemObserver {
    protected readonly fs: NodeFsaWatchFs;
    protected readonly callback: (records: IFileSystemChangeRecord[], observer: IFileSystemObserver) => void;
    protected readonly _observations: Map<IFileSystemSyncAccessHandle | IFileSystemFileHandle | IFileSystemDirectoryHandle, misc.IFSWatcher>;
    protected _records: IFileSystemChangeRecord[];
    protected _flushScheduled: boolean;
    constructor(fs: NodeFsaWatchFs, callback: (records: IFileSystemChangeRecord[], observer: IFileSystemObserver) => void);
    observe(handle: IFileSystemFileHandle | IFileSystemDirectoryHandle | IFileSystemSyncAccessHandle, options?: IFileSystemObserverObserveOptions): Promise<void>;
    unobserve(handle: IFileSystemFileHandle | IFileSystemDirectoryHandle | IFileSystemSyncAccessHandle): void;
    /** Disconnect and stop all observations. */
    disconnect(): void;
    protected onEvent(root: IFileSystemFileHandle | IFileSystemDirectoryHandle | IFileSystemSyncAccessHandle, watcher: misc.IFSWatcher, rootPath: string, isDirectory: boolean, ctx: NodeFsaContext, eventType: string, filename: string): Promise<void>;
    protected _handle(absolute: string, stats: misc.IStats, ctx: NodeFsaContext): IFileSystemHandle;
    protected _enqueue(record: IFileSystemChangeRecord): void;
}
