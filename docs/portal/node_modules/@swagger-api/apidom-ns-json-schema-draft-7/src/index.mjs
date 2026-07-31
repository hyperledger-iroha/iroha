export { isRefElement, isLinkElement, isMemberElement, isObjectElement, isArrayElement, isBooleanElement, isNullElement, isElement, isNumberElement, isStringElement } from '@swagger-api/apidom-core';
export { default as mediaTypes, JSONSchemaDraft7MediaTypes } from "./media-types.mjs";
// eslint-disable-next-line no-restricted-exports
export { default } from "./namespace.mjs";
export { default as refractorPluginReplaceEmptyElement } from "./refractor/plugins/replace-empty-element.mjs";
export { default as refract, createRefractor } from "./refractor/index.mjs";
export { default as specificationObj } from "./refractor/specification.mjs";
export { isJSONReferenceElement, isJSONSchemaElement, isLinkDescriptionElement } from "./predicates.mjs";
export { isJSONReferenceLikeElement, SpecificationVisitor, FallbackVisitor, FixedFieldsVisitor, PatternedFieldsVisitor, MapVisitor, AlternatingVisitor, ParentSchemaAwareVisitor, Visitor } from '@swagger-api/apidom-ns-json-schema-draft-6';
export { default as JSONSchemaVisitor } from "./refractor/visitors/json-schema/index.mjs";
export { default as LinkDescriptionVisitor } from "./refractor/visitors/json-schema/link-description/index.mjs";
export { keyMap, getNodeType } from "./traversal/visitor.mjs";
/**
 * JSON Schema Draft 7 specification elements.
 */
export { JSONSchemaElement, LinkDescriptionElement } from "./refractor/registration.mjs";
export { JSONReferenceElement } from '@swagger-api/apidom-ns-json-schema-draft-6';