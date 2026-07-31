import { Link } from '@jsonjoy.com/fs-core';
import * as opts from '@jsonjoy.com/fs-node-utils/lib/types/options';
import type { IDir, IDirent } from '@jsonjoy.com/fs-node-utils/lib/types/misc';
/**
 * A directory stream, like `fs.Dir`.
 */
export declare class Dir implements IDir {
    protected readonly link: Link;
    protected options: opts.IOpendirOptions;
    private iteratorInfo;
    private closed;
    private operationQueue;
    constructor(link: Link, options: opts.IOpendirOptions);
    private closeBase;
    private readBase;
    readonly path: string;
    close(): Promise<void>;
    close(callback?: (err?: Error) => void): void;
    closeSync(): void;
    read(): Promise<IDirent | null>;
    read(callback?: (err: Error | null, dir?: IDirent | null) => void): void;
    readSync(): IDirent | null;
    [Symbol.asyncIterator](): AsyncIterableIterator<IDirent>;
    [Symbol.asyncDispose](): Promise<void>;
    [Symbol.dispose](): void;
}
