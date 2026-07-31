"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _visitor = require("./visitor.cjs");
var _ephemeralArray = _interopRequireDefault(require("./ast/ephemeral-array.cjs"));
var _ephemeralObject = _interopRequireDefault(require("./ast/ephemeral-object.cjs"));
var _index = require("../../../predicates/index.cjs");
/* eslint-disable class-methods-use-this */

class Visitor {
  ObjectElement = {
    enter: element => {
      if (this.references.has(element)) {
        return this.references.get(element).toReference();
      }
      const ephemeral = new _ephemeralObject.default(element.content);
      this.references.set(element, ephemeral);
      return ephemeral;
    }
  };
  EphemeralObject = {
    leave: ephemeral => {
      return ephemeral.toObject();
    }
  };
  MemberElement = {
    enter: element => {
      return [element.key, element.value];
    }
  };
  ArrayElement = {
    enter: element => {
      if (this.references.has(element)) {
        return this.references.get(element).toReference();
      }
      const ephemeral = new _ephemeralArray.default(element.content);
      this.references.set(element, ephemeral);
      return ephemeral;
    }
  };
  EphemeralArray = {
    leave: ephemeral => {
      return ephemeral.toArray();
    }
  };
  references = new WeakMap();
  BooleanElement(element) {
    return element.toValue();
  }
  NumberElement(element) {
    return element.toValue();
  }
  StringElement(element) {
    return element.toValue();
  }
  NullElement() {
    return null;
  }
  RefElement(element, ...rest) {
    var _ancestors;
    const ancestors = rest[3];
    if (((_ancestors = ancestors[ancestors.length - 1]) == null ? void 0 : _ancestors.type) === 'EphemeralObject') {
      return Symbol.for('delete-node');
    }
    return String(element.toValue());
  }
  LinkElement(element) {
    if ((0, _index.isStringElement)(element.href)) {
      return element.href.toValue();
    }
    return '';
  }
}
/**
 * @public
 */
const serializer = element => {
  if (!(0, _index.isElement)(element)) return element;

  // shortcut optimization for certain element types
  if ((0, _index.isStringElement)(element) || (0, _index.isNumberElement)(element) || (0, _index.isBooleanElement)(element) || (0, _index.isNullElement)(element)) {
    return element.toValue();
  }
  return (0, _visitor.visit)(element, new Visitor());
};
var _default = exports.default = serializer;