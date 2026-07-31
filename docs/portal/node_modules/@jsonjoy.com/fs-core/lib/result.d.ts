export interface Ok<T> {
    ok: true;
    value: T;
}
export interface Err<E> {
    ok: false;
    err: E;
}
export type Result<T, E> = Ok<T> | Err<E>;
export declare function Ok<T>(value: T): Ok<T>;
export declare function Err<E>(err: E): Err<E>;
