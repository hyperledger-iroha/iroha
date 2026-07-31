"use strict";

exports.__esModule = true;
exports.test = exports.resolve = exports.percentEndingNormalizer = exports.pathSegmentNormalizer = exports.parse = exports.normalize = exports.isIdentical = exports.identityNormalizer = exports.encodePathComponent = exports.caseNormalizer = exports.Grammar = void 0;
var _pathTemplating = _interopRequireDefault(require("./path-templating.cjs"));
exports.Grammar = _pathTemplating.default;
var _test = _interopRequireDefault(require("./test.cjs"));
exports.test = _test.default;
var _index = _interopRequireDefault(require("./parse/index.cjs"));
exports.parse = _index.default;
var _resolve = _interopRequireWildcard(require("./resolve.cjs"));
exports.resolve = _resolve.default;
exports.encodePathComponent = _resolve.encodePathComponent;
var _index2 = _interopRequireWildcard(require("./normalization/index.cjs"));
exports.identityNormalizer = _index2.identityNormalizer;
exports.caseNormalizer = _index2.caseNormalizer;
exports.pathSegmentNormalizer = _index2.pathSegmentNormalizer;
exports.percentEndingNormalizer = _index2.percentEndingNormalizer;
exports.normalize = _index2.default;
var _isIdentical = _interopRequireDefault(require("./equivalence/is-identical.cjs"));
exports.isIdentical = _isIdentical.default;
function _getRequireWildcardCache(e) { if ("function" != typeof WeakMap) return null; var r = new WeakMap(), t = new WeakMap(); return (_getRequireWildcardCache = function (e) { return e ? t : r; })(e); }
function _interopRequireWildcard(e, r) { if (!r && e && e.__esModule) return e; if (null === e || "object" != typeof e && "function" != typeof e) return { default: e }; var t = _getRequireWildcardCache(r); if (t && t.has(e)) return t.get(e); var n = { __proto__: null }, a = Object.defineProperty && Object.getOwnPropertyDescriptor; for (var u in e) if ("default" !== u && {}.hasOwnProperty.call(e, u)) { var i = a ? Object.getOwnPropertyDescriptor(e, u) : null; i && (i.get || i.set) ? Object.defineProperty(n, u, i) : n[u] = e[u]; } return n.default = e, t && t.set(e, n), n; }
function _interopRequireDefault(e) { return e && e.__esModule ? e : { default: e }; }