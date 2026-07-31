import type { JavaScriptLinked } from '.';
import type { JavaScript } from './types';
export declare const compile: <T>(js: JavaScript<T>) => T;
export declare const compileClosure: <T>(fn: JavaScriptLinked<T, any>) => T;
