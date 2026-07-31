"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.JsExpression = void 0;
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
class JsExpression {
    constructor(expression) {
        this.expression = expression;
        this._wasUsed = false;
        this._listeners = [];
    }
    get wasUsed() {
        return this._wasUsed;
    }
    use() {
        if (this._wasUsed)
            return this._expression;
        this._wasUsed = true;
        const expression = (this._expression = this.expression());
        for (const listener of this._listeners)
            listener(expression);
        return expression;
    }
    chain(use) {
        return new JsExpression(() => use(this.use()));
    }
    addListener(listener) {
        this._listeners.push(listener);
    }
}
exports.JsExpression = JsExpression;
//# sourceMappingURL=JsExpression.js.map