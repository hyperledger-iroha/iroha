import type { TWatchIgnorePattern } from './types/options';
/** Converts a single `watch()` `ignore` pattern into a filename predicate. */
export declare const watchIgnorePatternToMatcher: (pattern: TWatchIgnorePattern) => ((filename: string) => boolean);
/** Converts the `watch()` `ignore` option (pattern or array of patterns) into a filename predicate. */
export declare const watchIgnoreToMatcher: (ignore: TWatchIgnorePattern | TWatchIgnorePattern[]) => ((filename: string) => boolean);
