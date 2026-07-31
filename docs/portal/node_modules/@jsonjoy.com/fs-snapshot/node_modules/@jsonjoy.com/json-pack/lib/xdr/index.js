"use strict";
/**
 * XDR (External Data Representation Standard) module
 *
 * Fully compliant with:
 * - RFC 4506 (May 2006) - Current standard with IANA and security considerations
 * - RFC 1832 (August 1995) - Enhanced standard with quadruple floats and optional-data
 * - RFC 1014 (June 1987) - Original standard
 *
 * Features:
 * - All XDR data types (int, hyper, float, double, string, opaque, arrays, structs, unions, optional-data)
 * - Big-endian byte order with 4-byte alignment
 * - Schema-based encoding/decoding with validation
 * - TypeScript definitions
 */
Object.defineProperty(exports, "__esModule", { value: true });
const tslib_1 = require("tslib");
tslib_1.__exportStar(require("./types"), exports);
tslib_1.__exportStar(require("./XdrEncoder"), exports);
tslib_1.__exportStar(require("./XdrDecoder"), exports);
tslib_1.__exportStar(require("./XdrSchemaEncoder"), exports);
tslib_1.__exportStar(require("./XdrSchemaDecoder"), exports);
tslib_1.__exportStar(require("./XdrSchemaValidator"), exports);
tslib_1.__exportStar(require("./XdrUnion"), exports);
//# sourceMappingURL=index.js.map