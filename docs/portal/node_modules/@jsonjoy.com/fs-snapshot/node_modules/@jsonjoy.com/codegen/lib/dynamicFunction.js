"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.dynamicFunction = void 0;
/**
 * Wraps a function into a proxy function with the same signature, but which can
 * be re-implemented by the user at runtime.
 *
 * @param implementation Initial implementation.
 * @returns Proxy function and implementation setter.
 */
const dynamicFunction = (implementation) => {
    const proxy = ((...args) => implementation(...args));
    const set = (f) => {
        implementation = f;
    };
    return [proxy, set];
};
exports.dynamicFunction = dynamicFunction;
//# sourceMappingURL=dynamicFunction.js.map