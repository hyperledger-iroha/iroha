"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = exports.createRefractor = void 0;
var _index = require("./plugins/dispatcher/index.cjs");
var _visitor = require("../traversal/visitor.cjs");
var _index2 = require("../clone/index.cjs");
var _index3 = require("../predicates/index.cjs");
var _toolbox = _interopRequireDefault(require("./toolbox.cjs"));
const refract = (value, {
  Type,
  plugins = []
}) => {
  /**
   * This is where values gets refracted into generic ApiDOM.
   * We don't allow consumers to hook into this translation.
   * Though we allow consumers to define their onw plugins on already transformed ApiDOM.
   */
  const element = new Type(value);
  if ((0, _index3.isElement)(value)) {
    if (value.meta.length > 0) {
      element.meta = (0, _index2.cloneDeep)(value.meta);
    }
    if (value.attributes.length > 0) {
      element.attributes = (0, _index2.cloneDeep)(value.attributes);
    }
  }

  /**
   * Run plugins only when necessary.
   * Running plugins visitors means extra single traversal === performance hit.
   */
  return (0, _index.dispatchPluginsSync)(element, plugins, {
    toolboxCreator: _toolbox.default,
    visitorOptions: {
      nodeTypeGetter: _visitor.getNodeType
    }
  });
};
const createRefractor = Type => (value, options = {}) => refract(value, {
  ...options,
  Type
});
exports.createRefractor = createRefractor;
var _default = exports.default = refract;