import { CoreWatcher } from '@jsonjoy.com/fs-core';
import type { Superblock } from '@jsonjoy.com/fs-core';
import type { IFileSystemChangeRecord, IFileSystemDirectoryHandle, IFileSystemFileHandle, IFileSystemHandle, IFileSystemObserver, IFileSystemObserverObserveOptions, IFileSystemSyncAccessHandle } from './types';
/**
 * @see https://developer.mozilla.org/en-US/docs/Web/API/FileSystemChangeRecord
 */
export declare class FileSystemChangeRecord implements IFileSystemChangeRecord {
    readonly root: IFileSystemHandle | IFileSystemSyncAccessHandle | IFileSystemDirectoryHandle;
    readonly type: IFileSystemChangeRecord['type'];
    readonly changedHandle: IFileSystemHandle | IFileSystemSyncAccessHandle | IFileSystemDirectoryHandle | null;
    readonly relativePathComponents: string[];
    readonly relativePathMovedFrom: string[] | null;
    constructor(root: IFileSystemHandle | IFileSystemSyncAccessHandle | IFileSystemDirectoryHandle, type: IFileSystemChangeRecord['type'], changedHandle: IFileSystemHandle | IFileSystemSyncAccessHandle | IFileSystemDirectoryHandle | null, relativePathComponents: string[], relativePathMovedFrom?: string[] | null);
}
/**
 * @see https://developer.mozilla.org/en-US/docs/Web/API/FileSystemObserver
 */
export declare class CoreFileSystemObserver implements IFileSystemObserver {
    protected readonly _core: Superblock;
    protected readonly callback: (records: IFileSystemChangeRecord[], observer: IFileSystemObserver) => void;
    protected readonly _observations: Map<IFileSystemDirectoryHandle | IFileSystemFileHandle | IFileSystemSyncAccessHandle, CoreWatcher>;
    protected _records: IFileSystemChangeRecord[];
    protected _flushScheduled: boolean;
    constructor(_core: Superblock, callback: (records: IFileSystemChangeRecord[], observer: IFileSystemObserver) => void);
    observe(handle: IFileSystemFileHandle | IFileSystemDirectoryHandle | IFileSystemSyncAccessHandle, options?: IFileSystemObserverObserveOptions): Promise<void>;
    unobserve(handle: IFileSystemFileHandle | IFileSystemDirectoryHandle | IFileSystemSyncAccessHandle): void;
    /** Disconnect and stop all observations. */
    disconnect(): void;
    private _onEvent;
    private _changedHandle;
    private _enqueue;
}
