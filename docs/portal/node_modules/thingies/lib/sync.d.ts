import { FanOut } from './fanout';
/** React.js synchronous state interface. */
export interface SyncStore<T> {
    subscribe: SyncStoreSubscribe;
    getSnapshot: () => T;
}
export type SyncStoreSubscribe = (callback: () => void) => SyncStoreUnsubscribe;
export type SyncStoreUnsubscribe = () => void;
export type SyncDep<T> = SyncValue<T> & SyncStore<T> & FanOut<void>;
export type WrapListInSyncDep<T extends unknown[]> = {
    [K in keyof T]: SyncDep<T[K]>;
};
export interface Disposable {
    dispose(): void;
}
export interface SyncValue<T> {
    value: T;
}
export declare class Value<V> extends FanOut<void> implements SyncStore<V>, SyncValue<V> {
    constructor(value: V);
    next(value: V, force?: boolean): void;
    /** ----------------------------------------------------- {@link SyncValue} */
    value: V;
    /** ----------------------------------------------------- {@link SyncStore} */
    readonly subscribe: SyncStoreSubscribe;
    readonly getSnapshot: () => V;
}
export declare class Computed<N, V extends unknown[] = any> extends FanOut<void> implements SyncValue<N>, SyncStore<N>, Disposable {
    protected readonly deps: WrapListInSyncDep<V>;
    protected readonly compute: (args: V) => N;
    private cache;
    private subs;
    constructor(deps: WrapListInSyncDep<V>, compute: (args: V) => N);
    private _comp;
    /** ----------------------------------------------------- {@link SyncValue} */
    get value(): N;
    /** ----------------------------------------------------- {@link SyncStore} */
    readonly subscribe: SyncStoreSubscribe;
    readonly getSnapshot: () => N;
    /** ---------------------------------------------------- {@link Disposable} */
    dispose(): void;
}
export declare const val: <V>(initial: V) => Value<V>;
export declare const comp: <V extends unknown[], N>(deps: WrapListInSyncDep<V>, compute: (args: V) => N) => Computed<N, V>;
