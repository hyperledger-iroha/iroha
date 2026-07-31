/**
 * Switcher for code generation. It first executes "evaluation" function
 * 3 times, and then generates optimized code.
 */
export declare const createSwitch: <F extends (...args: any[]) => any>(fn: F, codegen: () => F) => F;
