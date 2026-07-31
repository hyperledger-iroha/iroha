import { Link } from '@jsonjoy.com/fs-core';
import { TEncodingExtended, TDataOut } from '@jsonjoy.com/fs-node-utils';
import type { IDirent } from '@jsonjoy.com/fs-node-utils/lib/types/misc';
/**
 * A directory entry, like `fs.Dirent`.
 */
export declare class Dirent implements IDirent {
    static build(link: Link, encoding: TEncodingExtended | undefined): Dirent;
    name: TDataOut;
    parentPath: string;
    private mode;
    /**
     * @deprecated Will be removed at any time.
     * @see https://nodejs.org/api/deprecations.html#DEP0178
     */
    path: string;
    private _checkModeProperty;
    isDirectory(): boolean;
    isFile(): boolean;
    isBlockDevice(): boolean;
    isCharacterDevice(): boolean;
    isSymbolicLink(): boolean;
    isFIFO(): boolean;
    isSocket(): boolean;
}
export default Dirent;
