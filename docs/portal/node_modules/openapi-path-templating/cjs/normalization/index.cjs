"use strict";

exports.__esModule = true;
exports.identityNormalizer = exports.default = void 0;
var _percentEncoding = _interopRequireDefault(require("./percent-encoding.cjs"));
exports.percentEndingNormalizer = _percentEncoding.default;
var _case = _interopRequireDefault(require("./case.cjs"));
exports.caseNormalizer = _case.default;
var _pathSegment = _interopRequireDefault(require("./path-segment.cjs"));
exports.pathSegmentNormalizer = _pathSegment.default;
var _identity = _interopRequireDefault(require("./identity.cjs"));
exports.identityNormalizer = _identity.default;
function _interopRequireDefault(e) { return e && e.__esModule ? e : { default: e }; }
/**
 * Implementing https://datatracker.ietf.org/doc/html/rfc3986#section-6.2
 */

const normalize = pathTemplate => {
  const decodedPath = (0, _percentEncoding.default)(pathTemplate);
  const caseNormalizedPath = (0, _case.default)(decodedPath);
  return (0, _pathSegment.default)(caseNormalizedPath);
};
var _default = exports.default = normalize;