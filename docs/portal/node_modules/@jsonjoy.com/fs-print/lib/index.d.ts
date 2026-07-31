import { FsSynchronousApi } from '@jsonjoy.com/fs-node-utils';
export declare const toTreeSync: (fs: FsSynchronousApi, opts?: ToTreeOptions) => string;
export interface ToTreeOptions {
    dir?: string;
    tab?: string;
    depth?: number;
    separator?: '/' | '\\';
    sort?: boolean;
}
