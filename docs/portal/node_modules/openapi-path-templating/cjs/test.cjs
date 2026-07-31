"use strict";

exports.__esModule = true;
exports.default = void 0;
var _index = _interopRequireDefault(require("./parse/index.cjs"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { default: e }; }
/**
 * Test if a string is a path template.
 *
 * @param {string} pathTemplate
 * @param {Object} [options={}] - An object.
 * @param {boolean} [options.strict=true] - A boolean indicating presence of at least one `template-expression` AST node.
 * @returns {boolean}
 */
const test = (pathTemplate, {
  strict = false
} = {}) => {
  try {
    const parseResult = (0, _index.default)(pathTemplate);
    if (!parseResult.result.success) return false;
    if (!strict) return true;
    const parts = [];
    parseResult.ast.translate(parts);
    return parts.some(([type]) => type === 'template-expression');
  } catch {
    return false;
  }
};
var _default = exports.default = test;