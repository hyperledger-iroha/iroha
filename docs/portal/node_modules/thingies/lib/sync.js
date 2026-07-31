"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.comp = exports.val = exports.Computed = exports.Value = void 0;
const fanout_1 = require("./fanout");
const NO_CACHE = Symbol();
class Value extends fanout_1.FanOut {
    constructor(value) {
        super();
        /** ----------------------------------------------------- {@link SyncStore} */
        this.subscribe = (cb) => this.listen(cb);
        this.getSnapshot = () => this.value;
        this.value = value;
    }
    next(value, force = false) {
        if (!force && this.value === value)
            return;
        this.value = value;
        this.emit();
    }
}
exports.Value = Value;
class Computed extends fanout_1.FanOut {
    constructor(deps, compute) {
        super();
        this.deps = deps;
        this.compute = compute;
        this.cache = NO_CACHE;
        /** ----------------------------------------------------- {@link SyncStore} */
        this.subscribe = (cb) => this.listen(cb);
        this.getSnapshot = () => this._comp();
        const subs = (this.subs = []);
        const length = deps.length;
        for (let i = 0; i < length; i++) {
            const dep = deps[i];
            const sub = dep.listen(() => {
                this.cache = NO_CACHE;
                this.emit();
            });
            subs.push(sub);
        }
    }
    _comp() {
        const cached = this.cache;
        if (cached !== NO_CACHE)
            return cached;
        return (this.cache = this.compute(this.deps.map((dep) => dep.getSnapshot())));
    }
    /** ----------------------------------------------------- {@link SyncValue} */
    get value() {
        return this._comp();
    }
    /** ---------------------------------------------------- {@link Disposable} */
    dispose() {
        for (const sub of this.subs)
            sub();
    }
}
exports.Computed = Computed;
const val = (initial) => new Value(initial);
exports.val = val;
const comp = (deps, compute) => new Computed(deps, compute);
exports.comp = comp;
