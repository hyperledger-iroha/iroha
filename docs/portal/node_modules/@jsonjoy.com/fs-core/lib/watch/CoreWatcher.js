"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.CoreWatcher = exports.CoreWatchEvent = void 0;
const fanout_1 = require("thingies/lib/fanout");
/**
 * A change event scoped to a {@link CoreWatcher} watch root. Unlike the
 * volume-global {@link FsEvent}, `steps` (and `oldSteps`) are relative to the
 * watch root; an empty `steps` array means the watch target itself changed.
 */
class CoreWatchEvent {
    constructor(type, 
    /** Path relative to the watch root, empty for the watch target itself. */
    steps, node, link, 
    /** Former path relative to the watch root, set only for in-scope moves. */
    oldSteps = void 0) {
        this.type = type;
        this.steps = steps;
        this.node = node;
        this.link = link;
        this.oldSteps = oldSteps;
    }
}
exports.CoreWatchEvent = CoreWatchEvent;
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
class CoreWatcher {
    /** @throws ENOENT-style error when the watch target does not exist. */
    constructor(core, path, opts = {}) {
        /** Fan-out of scoped change events. Multiple consumers may subscribe. */
        this.changes = new fanout_1.FanOut();
        this.link = (opts.follow ?? true) ? core.getResolvedLinkOrThrow(path, 'watch') : core.getLinkOrThrow(path, 'watch');
        this.node = this.link.getNode();
        this.recursive = !!opts.recursive;
        this.unsub = core.changes.listen(event => this.onEvent(event));
    }
    get closed() {
        return !this.unsub;
    }
    onEvent(event) {
        const rootLength = this.link.steps.length;
        const isSelf = event.link === this.link ||
            (event.node === this.node && (event.type === 2 /* FsEventType.MODIFY */ || event.type === 3 /* FsEventType.ATTRIB */));
        if (isSelf) {
            this.changes.emit(new CoreWatchEvent(event.type, [], event.node, this.link));
            if (event.type === 1 /* FsEventType.DELETE */)
                this.close();
            return;
        }
        if (event.type === 4 /* FsEventType.MOVE */) {
            const from = event.oldSteps && this.inScope(event.oldSteps) ? event.oldSteps.slice(rootLength) : undefined;
            const to = this.inScope(event.steps) ? event.steps.slice(rootLength) : undefined;
            if (from && to)
                this.changes.emit(new CoreWatchEvent(4 /* FsEventType.MOVE */, to, event.node, event.link, from));
            else if (from)
                this.changes.emit(new CoreWatchEvent(1 /* FsEventType.DELETE */, from, event.node, event.link));
            else if (to)
                this.changes.emit(new CoreWatchEvent(0 /* FsEventType.CREATE */, to, event.node, event.link));
            return;
        }
        if (this.inScope(event.steps))
            this.changes.emit(new CoreWatchEvent(event.type, event.steps.slice(rootLength), event.node, event.link));
    }
    inScope(steps) {
        const root = this.link.steps;
        if (steps.length <= root.length)
            return false;
        if (!this.recursive && steps.length !== root.length + 1)
            return false;
        for (let i = 0; i < root.length; i++)
            if (steps[i] !== root[i])
                return false;
        return true;
    }
    /** Stop watching and unsubscribe from the volume change stream. */
    close() {
        this.unsub?.();
        this.unsub = undefined;
    }
}
exports.CoreWatcher = CoreWatcher;
//# sourceMappingURL=CoreWatcher.js.map