"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apgLite = require("../apg-lite.cjs");
var _pathTemplating = _interopRequireDefault(require("../path-templating.cjs"));
var _index = _interopRequireDefault(require("../parse/index.cjs"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { default: e }; }
/**
 * Implementation of https://datatracker.ietf.org/doc/html/rfc3986#section-6.2.2.2
 */

const significantTypes = ['slash', 'path-literal', 'template-expression'];
const grammar = new _pathTemplating.default();
const parser = new _apgLite.Parser();
const percentEndingNormalizer = pathTemplate => {
  const parseResult = (0, _index.default)(pathTemplate);
  if (!parseResult.result.success) return pathTemplate;
  const parts = [];
  parseResult.ast.translate(parts);
  return parts.reduce((pathTemplateNormalized, [type, value]) => {
    let normalizedValue = value;
    if (type === 'path-literal') {
      normalizedValue = value.replace(/%[0-9a-fA-F]{2}/g, match => {
        try {
          const char = decodeURIComponent(match);
          const {
            success
          } = parser.parse(grammar, 'unreserved', char);
          return success ? char : match;
        } catch {
          return match;
        }
      });
    }
    if (significantTypes.includes(type)) {
      return `${pathTemplateNormalized}${normalizedValue}`;
    }
    return pathTemplateNormalized;
  }, '');
};
var _default = exports.default = percentEndingNormalizer;