"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _YamlSchemaError = _interopRequireDefault(require("./YamlSchemaError.cjs"));
/**
 * @public
 */

/**
 * @public
 */
class YamlTagError extends _YamlSchemaError.default {
  specificTagName;
  explicitTagName;
  tagKind;
  tagStartPositionRow;
  tagStartPositionColumn;
  tagStartPositionIndex;
  tagEndPositionRow;
  tagEndPositionColumn;
  tagEndPositionIndex;
  nodeCanonicalContent;
  node;
  constructor(message, structuredOptions) {
    super(message, structuredOptions);
    if (typeof structuredOptions !== 'undefined') {
      this.specificTagName = structuredOptions.specificTagName;
      this.explicitTagName = structuredOptions.explicitTagName;
      this.tagKind = structuredOptions.tagKind;
      this.tagStartPositionRow = structuredOptions.tagStartPositionRow;
      this.tagStartPositionColumn = structuredOptions.tagStartPositionColumn;
      this.tagStartPositionIndex = structuredOptions.tagStartPositionIndex;
      this.tagEndPositionRow = structuredOptions.tagEndPositionRow;
      this.tagEndPositionColumn = structuredOptions.tagEndPositionColumn;
      this.tagEndPositionIndex = structuredOptions.tagEndPositionIndex;
      this.nodeCanonicalContent = structuredOptions.nodeCanonicalContent;
      this.node = structuredOptions.node;
    }
  }
}
var _default = exports.default = YamlTagError;