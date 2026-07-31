"use strict";
var __createBinding = (this && this.__createBinding) || (Object.create ? (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    var desc = Object.getOwnPropertyDescriptor(m, k);
    if (!desc || ("get" in desc ? !m.__esModule : desc.writable || desc.configurable)) {
      desc = { enumerable: true, get: function() { return m[k]; } };
    }
    Object.defineProperty(o, k2, desc);
}) : (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    o[k2] = m[k];
}));
var __setModuleDefault = (this && this.__setModuleDefault) || (Object.create ? (function(o, v) {
    Object.defineProperty(o, "default", { enumerable: true, value: v });
}) : function(o, v) {
    o["default"] = v;
});
var __importStar = (this && this.__importStar) || (function () {
    var ownKeys = function(o) {
        ownKeys = Object.getOwnPropertyNames || function (o) {
            var ar = [];
            for (var k in o) if (Object.prototype.hasOwnProperty.call(o, k)) ar[ar.length] = k;
            return ar;
        };
        return ownKeys(o);
    };
    return function (mod) {
        if (mod && mod.__esModule) return mod;
        var result = {};
        if (mod != null) for (var k = ownKeys(mod), i = 0; i < k.length; i++) if (k[i] !== "default") __createBinding(result, mod, k[i]);
        __setModuleDefault(result, mod);
        return result;
    };
})();
const bonjour_1 = __importStar(require("./lib/bonjour"));
class Bonjour extends bonjour_1.default {
}
const BonjourConstructor = Bonjour;
(function (Bonjour_1) {
    Bonjour_1.Bonjour = BonjourConstructor;
    Bonjour_1.Service = bonjour_1.Service;
    Bonjour_1.Browser = bonjour_1.Browser;
})(Bonjour || (Bonjour = {}));
Object.defineProperty(Bonjour, 'default', {
    enumerable: true,
    value: Bonjour,
});
module.exports.Bonjour = Bonjour;
module.exports.Service = bonjour_1.Service;
module.exports.Browser = bonjour_1.Browser;
module.exports.default = Bonjour;
module.exports = Bonjour;
//# sourceMappingURL=index.js.map