"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.watchIgnoreToMatcher = exports.watchIgnorePatternToMatcher = void 0;
const glob_to_regex_js_1 = require("glob-to-regex.js");
/** Converts a single `watch()` `ignore` pattern into a filename predicate. */
const watchIgnorePatternToMatcher = (pattern) => {
    if (typeof pattern === 'function')
        return pattern;
    if (pattern instanceof RegExp)
        return filename => pattern.test(filename);
    if (typeof pattern === 'string') {
        const regex = (0, glob_to_regex_js_1.toRegex)(pattern);
        return filename => regex.test(filename);
    }
    throw new TypeError('The "options.ignore" property must be of type string, RegExp, function, or an array thereof. ' +
        `Received ${typeof pattern}`);
};
exports.watchIgnorePatternToMatcher = watchIgnorePatternToMatcher;
/** Converts the `watch()` `ignore` option (pattern or array of patterns) into a filename predicate. */
const watchIgnoreToMatcher = (ignore) => {
    if (Array.isArray(ignore)) {
        const matchers = ignore.map(exports.watchIgnorePatternToMatcher);
        return filename => matchers.some(matcher => matcher(filename));
    }
    return (0, exports.watchIgnorePatternToMatcher)(ignore);
};
exports.watchIgnoreToMatcher = watchIgnoreToMatcher;
//# sourceMappingURL=watchIgnore.js.map