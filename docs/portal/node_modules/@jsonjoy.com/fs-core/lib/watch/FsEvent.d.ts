import type { Node } from '../Node';
import type { Link } from '../Link';
export declare const enum FsEventType {
    /** A directory entry appeared: file, directory, symlink, or new hard link. */
    CREATE = 0,
    /** A directory entry was removed. */
    DELETE = 1,
    /** File content changed (write, truncate). */
    MODIFY = 2,
    /** File metadata changed: permissions, ownership, or timestamps. */
    ATTRIB = 3,
    /** An entry was renamed/moved within the tree. */
    MOVE = 4
}
export declare class FsEvent {
    readonly type: FsEventType;
    readonly steps: string[];
    readonly node: Node;
    readonly link: Link;
    readonly oldSteps: string[] | undefined;
    constructor(type: FsEventType, steps: string[], node: Node, link: Link, oldSteps?: string[] | undefined);
}
