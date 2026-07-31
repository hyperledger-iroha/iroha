import type { PathLike } from '@jsonjoy.com/fs-node-utils/lib/types/misc';
export type TFileId = PathLike | number;
export interface StatError {
    code: string;
    message: string;
    path?: string;
    toError(): Error;
}
