import { visit } from "./visitor.mjs";
class Visitor {
  parentEdges;
  parent;
  constructor() {
    this.parentEdges = new WeakMap();
  }
  ObjectElement(objectElement) {
    this.parentEdges.set(objectElement, this.parent);
    this.parent = objectElement;
  }
  ArrayElement(arrayElement) {
    this.parentEdges.set(arrayElement, this.parent);
    this.parent = arrayElement;
  }
  MemberElement(memberElement) {
    this.parentEdges.set(memberElement, this.parent);
    this.parent = memberElement;
  }
  enter(element) {
    this.parentEdges.set(element, this.parent);
  }
}

/**
 * Computes upwards edges from every child to its parent.
 * @public
 */
const parents = element => {
  const visitor = new Visitor();
  visit(element, visitor);
  return visitor.parentEdges;
};
export default parents;