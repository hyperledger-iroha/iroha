"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.parse = exports.namespace = exports.mediaTypes = exports.detect = void 0;
var _node = _interopRequireDefault(require("./lexical-analysis/node.cjs"));
exports.lexicalAnalysis = _node.default;
var _index = _interopRequireDefault(require("./syntactic-analysis/direct/index.cjs"));
exports.syntacticAnalysis = _index.default;
exports.syntacticAnalysisDirect = _index.default;
var _index2 = _interopRequireDefault(require("./syntactic-analysis/indirect/index.cjs"));
exports.syntacticAnalysisIndirect = _index2.default;
var _adapter = require("./adapter.cjs");
exports.detectionRegExp = _adapter.detectionRegExp;
exports.mediaTypes = _adapter.mediaTypes;
exports.namespace = _adapter.namespace;
/**
 * @public
 */
const detect = async source => {
  if (!_adapter.detectionRegExp.test(source)) {
    return false;
  }
  try {
    const cst = await (0, _node.default)(source);
    return cst.rootNode.type !== 'ERROR';
  } catch {
    return false;
  }
};

/**
 * @public
 */

/**
 * @public
 */
exports.detect = detect;
/**
 * @public
 */
const parse = async (source, {
  sourceMap = false,
  syntacticAnalysis = 'direct'
} = {}) => {
  const cst = await (0, _node.default)(source);
  let apiDOM;
  if (syntacticAnalysis === 'indirect') {
    apiDOM = (0, _index2.default)(cst, {
      sourceMap
    });
  } else {
    apiDOM = (0, _index.default)(cst, {
      sourceMap
    });
  }
  return apiDOM;
};
exports.parse = parse;