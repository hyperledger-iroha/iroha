import { Mixin } from 'ts-mixer';
import { ObjectElement } from '@swagger-api/apidom-core';
import FallbackVisitor from "../FallbackVisitor.mjs";
import MapVisitor from "../generics/MapVisitor.mjs";
import ParentSchemaAwareVisitor from "./ParentSchemaAwareVisitor.mjs";
import { isJSONReferenceLikeElement } from "../../predicates.mjs";
/**
 * @public
 */
/**
 * @public
 */
class PropertiesVisitor extends Mixin(MapVisitor, ParentSchemaAwareVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new ObjectElement();
    this.element.classes.push('json-schema-properties');
    this.specPath = element => isJSONReferenceLikeElement(element) ? ['document', 'objects', 'JSONReference'] : ['document', 'objects', 'JSONSchema'];
  }
}
export default PropertiesVisitor;