"use strict";

exports.__esModule = true;
exports.default = void 0;
var _index = _interopRequireDefault(require("./parse/index.cjs"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { default: e }; }
/**
 * Test if a string is a server URL template.
 *
 * @param {string} serverURLTemplate
 * @param {Object} [options={}] - An object.
 * @param {boolean} [options.strict=true] - A boolean indicating presence of at least one `Server Variable` AST node.
 * @returns {boolean}
 */
const test = (serverURLTemplate, {
  strict = false
} = {}) => {
  try {
    const parseResult = (0, _index.default)(serverURLTemplate);
    if (!parseResult.result.success) return false;
    const parts = [];
    parseResult.ast.translate(parts);
    const hasServerVariable = parts.some(([type]) => type === 'server-variable');
    if (!strict && !hasServerVariable) {
      try {
        new URL(serverURLTemplate, 'https://vladimirgorej.com');
        return true;
      } catch {
        return false;
      }
    }
    return strict ? hasServerVariable : true;
  } catch {
    return false;
  }
};
var _default = exports.default = test;