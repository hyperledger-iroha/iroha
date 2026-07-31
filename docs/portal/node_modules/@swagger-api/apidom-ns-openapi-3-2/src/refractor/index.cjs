"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = exports.createRefractor = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
var _ramda = require("ramda");
var _specification = _interopRequireDefault(require("./specification.cjs"));
var _visitor = require("../traversal/visitor.cjs");
var _toolbox = _interopRequireDefault(require("./toolbox.cjs"));
const refract = (value, {
  specPath = ['visitors', 'document', 'objects', 'OpenApi', '$visitor'],
  plugins = []
} = {}) => {
  const element = (0, _apidomCore.refract)(value);
  const resolvedSpec = (0, _apidomCore.dereference)(_specification.default);

  /**
   * This is where generic ApiDOM becomes semantic (namespace applied).
   * We don't allow consumers to hook into this translation.
   * Though we allow consumers to define their onw plugins on already transformed ApiDOM.
   */
  const RootVisitorClass = (0, _ramda.path)(specPath, resolvedSpec);
  const rootVisitor = new RootVisitorClass({
    specObj: resolvedSpec
  });
  (0, _apidomCore.visit)(element, rootVisitor);

  /**
   * Running plugins visitors means extra single traversal === performance hit.
   */
  return (0, _apidomCore.dispatchRefractorPlugins)(rootVisitor.element, plugins, {
    toolboxCreator: _toolbox.default,
    visitorOptions: {
      keyMap: _visitor.keyMap,
      nodeTypeGetter: _visitor.getNodeType
    }
  });
};
const createRefractor = specPath => (value, options = {}) => refract(value, {
  specPath,
  ...options
});
exports.createRefractor = createRefractor;
var _default = exports.default = refract;