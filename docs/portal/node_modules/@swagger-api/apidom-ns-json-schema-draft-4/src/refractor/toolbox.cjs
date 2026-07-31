"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
var _interopRequireWildcard = require("@babel/runtime-corejs3/helpers/interopRequireWildcard").default;
exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
var jsonSchemaDraft4Predicates = _interopRequireWildcard(require("../predicates.cjs"));
var _namespace = _interopRequireDefault(require("../namespace.cjs"));
const createToolbox = () => {
  const namespace = (0, _apidomCore.createNamespace)(_namespace.default);
  const predicates = {
    ...jsonSchemaDraft4Predicates,
    isStringElement: _apidomCore.isStringElement
  };
  return {
    predicates,
    namespace
  };
};
var _default = exports.default = createToolbox;