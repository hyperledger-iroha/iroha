import { visit } from "./visitor.mjs";
import EphemeralArray from "./ast/ephemeral-array.mjs";
import EphemeralObject from "./ast/ephemeral-object.mjs";
import { isElement, isBooleanElement, isNumberElement, isStringElement, isNullElement } from "../../../predicates/index.mjs";
/* eslint-disable class-methods-use-this */
class Visitor {
  ObjectElement = {
    enter: element => {
      if (this.references.has(element)) {
        return this.references.get(element).toReference();
      }
      const ephemeral = new EphemeralObject(element.content);
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
      const ephemeral = new EphemeralArray(element.content);
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
    if (((_ancestors = ancestors[ancestors.length - 1]) === null || _ancestors === void 0 ? void 0 : _ancestors.type) === 'EphemeralObject') {
      return Symbol.for('delete-node');
    }
    return String(element.toValue());
  }
  LinkElement(element) {
    if (isStringElement(element.href)) {
      return element.href.toValue();
    }
    return '';
  }
}
/**
 * @public
 */
const serializer = element => {
  if (!isElement(element)) return element;

  // shortcut optimization for certain element types
  if (isStringElement(element) || isNumberElement(element) || isBooleanElement(element) || isNullElement(element)) {
    return element.toValue();
  }
  return visit(element, new Visitor());
};
export default serializer;