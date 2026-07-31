/**
 * JsExpression monad allows to write JS expression as strings which depend on each
 * other and tracks whether an expression was used or not.
 *
 * ```ts
 * const expr = new JsExpression(() => 'r0');
 * const subExpr = expr.chain((expr) => `${expr}["key"]`);
 *
 * expr.wasUsed; // false
 * subExpr.use(); // r0["key"]
 * expr.wasUsed; // true
 * subExpr.wasUsed; // true
 * ```
 */
export declare class JsExpression {
    private expression;
    private _wasUsed;
    private _expression?;
    private _listeners;
    constructor(expression: () => string);
    get wasUsed(): boolean;
    use(): string;
    chain(use: (expr: string) => string): JsExpression;
    addListener(listener: (expr: string) => void): void;
}
