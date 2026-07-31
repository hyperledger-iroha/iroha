export { isRefElement, isLinkElement, isMemberElement, isObjectElement, isArrayElement, isBooleanElement, isNullElement, isElement, isNumberElement, isStringElement } from '@swagger-api/apidom-core';
export { default as mediaTypes, JSONSchemaDraft4MediaTypes } from "./media-types.mjs";
// eslint-disable-next-line no-restricted-exports
export { default } from "./namespace.mjs";
export { default as refractorPluginReplaceEmptyElement } from "./refractor/plugins/replace-empty-element.mjs";
export { default as refract, createRefractor } from "./refractor/index.mjs";
export { default as specificationObj } from "./refractor/specification.mjs";
export { isJSONReferenceElement, isJSONSchemaElement, isLinkDescriptionElement, isMediaElement } from "./predicates.mjs";
export { isJSONReferenceLikeElement } from "./refractor/predicates.mjs";
export { default as Visitor } from "./refractor/visitors/Visitor.mjs";
export { default as SpecificationVisitor } from "./refractor/visitors/SpecificationVisitor.mjs";
export { default as FallbackVisitor } from "./refractor/visitors/FallbackVisitor.mjs";
export { default as FixedFieldsVisitor } from "./refractor/visitors/generics/FixedFieldsVisitor.mjs";
export { default as PatternedFieldsVisitor } from "./refractor/visitors/generics/PatternedFieldsVisitor.mjs";
export { default as MapVisitor } from "./refractor/visitors/generics/MapVisitor.mjs";
export { default as AlternatingVisitor } from "./refractor/visitors/generics/AlternatingVisitor.mjs";
export { default as ParentSchemaAwareVisitor } from "./refractor/visitors/json-schema/ParentSchemaAwareVisitor.mjs";
export { default as LinkDescriptionVisitor } from "./refractor/visitors/json-schema/link-description/index.mjs";
export { default as JSONSchemaVisitor } from "./refractor/visitors/json-schema/index.mjs";
export { default as ItemsVisitor } from "./refractor/visitors/json-schema/ItemsVisitor.mjs";
export { keyMap, getNodeType } from "./traversal/visitor.mjs";
/**
 * JSON Schema Draft 4 specification elements.
 */
export { JSONSchemaElement, JSONReferenceElement, MediaElement, LinkDescriptionElement } from "./refractor/registration.mjs";