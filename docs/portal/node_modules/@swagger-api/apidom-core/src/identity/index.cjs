"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.defaultIdentityManager = exports.IdentityManager = void 0;
var _minim = require("minim");
var _shortUniqueId = _interopRequireDefault(require("short-unique-id"));
var _ElementIdentityError = _interopRequireDefault(require("./errors/ElementIdentityError.cjs"));
var _index = require("../predicates/index.cjs");
/**
 * @public
 */
class IdentityManager {
  uuid;
  identityMap;
  constructor({
    length = 6
  } = {}) {
    this.uuid = new _shortUniqueId.default({
      length
    });
    this.identityMap = new WeakMap();
  }
  identify(element) {
    if (!(0, _index.isElement)(element)) {
      throw new _ElementIdentityError.default('Cannot not identify the element. `element` is neither structurally compatible nor a subclass of an Element class.', {
        value: element
      });
    }

    // use already assigned identity
    if (element.meta.hasKey('id') && (0, _index.isStringElement)(element.meta.get('id')) && !element.meta.get('id').equals('')) {
      return element.id;
    }

    // assign identity in immutable way
    if (this.identityMap.has(element)) {
      return this.identityMap.get(element);
    }

    // return element identity
    const id = new _minim.StringElement(this.generateId());
    this.identityMap.set(element, id);
    return id;
  }
  forget(element) {
    if (this.identityMap.has(element)) {
      this.identityMap.delete(element);
      return true;
    }
    return false;
  }
  generateId() {
    return this.uuid.randomUUID();
  }
}

/**
 * @public
 */
exports.IdentityManager = IdentityManager;
const defaultIdentityManager = exports.defaultIdentityManager = new IdentityManager();