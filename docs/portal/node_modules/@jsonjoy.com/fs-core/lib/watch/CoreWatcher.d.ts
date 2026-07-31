import { FanOut } from 'thingies/lib/fanout';
import { FsEventType } from './FsEvent';
import type { Link } from '../Link';
import type { Node } from '../Node';
import type { Superblock } from '../Superblock';
export interface CoreWatchOptions {
    /**
     * When `true`, the whole subtree of the watch target is observed. Otherwise
     * only the target itself and its direct children are (FSA depth-1 and inotify
     * semantics).
     */
    recursive?: boolean;
    /**
     * Resolve symlinks when locating the watch target, like `fs.watch` does.
     * When `false`, the symlink itself is watched. Default `true`.
     */
    follow?: boolean;
}
/**
 * A change event scoped to a {@link CoreWatcher} watch root. Unlike the
 * volume-global {@link FsEvent}, `steps` (and `oldSteps`) are relative to the
 * watch root; an empty `steps` array means the watch target itself changed.
 */
export declare class CoreWatchEvent {
    readonly type: FsEventType;
    /** Path relative to the watch root, empty for the watch target itself. */
    readonly steps: string[];
    readonly node: Node;
    readonly link: Link;
    /** Former path relative to the watch root, set only for in-scope moves. */
    readonly oldSteps: string[] | undefined;
    constructor(type: FsEventType, 
    /** Path relative to the watch root, empty for the watch target itself. */
    steps: string[], node: Node, link: Link, 
    /** Former path relative to the watch root, set only for in-scope moves. */
    oldSteps?: string[] | undefined);
}
/**
 * Subscribes to a {@link Superblock} change stream and re-emits only the
 * events that fall within the scope of one watched file or directory, with
 * paths rewritten to be relative to the watch root.
 *
 * Scope-boundary crossing moves are translated: a move into scope is emitted
 * as `CREATE`, a move out of scope as `DELETE`, and a move within scope stays
 * a `MOVE` carrying relative `oldSteps`. A `MOVE` with empty `steps` means the
 * watch target itself was renamed; watching continues at the new location
 * (the current absolute path is always `watcher.link.steps`). A `DELETE` with
 * empty `steps` is terminal: the watch target is gone and the watcher closes
 * itself after delivering it.
 */
export declare class CoreWatcher {
    /** The link of the watch target; `link.steps` stays current across renames. */
    readonly link: Link;
    /** The node the watch target pointed to when watching started. */
    readonly node: Node;
    /** Fan-out of scoped change events. Multiple consumers may subscribe. */
    readonly changes: FanOut<CoreWatchEvent>;
    private readonly recursive;
    private unsub;
    /** @throws ENOENT-style error when the watch target does not exist. */
    constructor(core: Superblock, path: string, opts?: CoreWatchOptions);
    get closed(): boolean;
    private onEvent;
    private inScope;
    /** Stop watching and unsubscribe from the volume change stream. */
    close(): void;
}
