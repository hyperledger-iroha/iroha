"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.XdrUnion = void 0;
/**
 * XDR Union data type that contains a discriminant and value.
 * Used for encoding XDR union types where the discriminant determines
 * which arm of the union is active.
 */
class XdrUnion {
    constructor(discriminant, value) {
        this.discriminant = discriminant;
        this.value = value;
    }
}
exports.XdrUnion = XdrUnion;
//# sourceMappingURL=XdrUnion.js.map