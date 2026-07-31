export { isRefElement, isLinkElement, isMemberElement, isObjectElement, isArrayElement, isBooleanElement, isNullElement, isElement, isNumberElement, isStringElement } from '@swagger-api/apidom-core';
export { default as mediaTypes, JSONSchema202012MediaTypes } from "./media-types.mjs";
// eslint-disable-next-line no-restricted-exports
export { default } from "./namespace.mjs";
export { default as refractorPluginReplaceEmptyElement } from "./refractor/plugins/replace-empty-element.mjs";
export { default as refract, createRefractor } from "./refractor/index.mjs";
export { default as specificationObj } from "./refractor/specification.mjs";
export { isJSONSchemaElement, isLinkDescriptionElement } from "./predicates.mjs";
export { SpecificationVisitor, FallbackVisitor, FixedFieldsVisitor, PatternedFieldsVisitor, MapVisitor, AlternatingVisitor, ParentSchemaAwareVisitor, Visitor } from '@swagger-api/apidom-ns-json-schema-2019-09';
export { default as JSONSchemaVisitor } from "./refractor/visitors/json-schema/index.mjs";
export { default as LinkDescriptionVisitor } from "./refractor/visitors/json-schema/link-description/index.mjs";
export { $defsVisitor } from '@swagger-api/apidom-ns-json-schema-2019-09';
export { $refVisitor } from '@swagger-api/apidom-ns-json-schema-2019-09';
export { $vocabularyVisitor } from '@swagger-api/apidom-ns-json-schema-2019-09';
export { AllOfVisitor } from '@swagger-api/apidom-ns-json-schema-2019-09';
export { AnyOfVisitor } from '@swagger-api/apidom-ns-json-schema-2019-09';
export { DependentRequiredVisitor } from '@swagger-api/apidom-ns-json-schema-2019-09';
export { DependentSchemasVisitor } from '@swagger-api/apidom-ns-json-schema-2019-09';
export { ItemsVisitor } from '@swagger-api/apidom-ns-json-schema-2019-09';
export { OneOfVisitor } from '@swagger-api/apidom-ns-json-schema-2019-09';
export { PatternPropertiesVisitor } from '@swagger-api/apidom-ns-json-schema-2019-09';
export { PropertiesVisitor } from '@swagger-api/apidom-ns-json-schema-2019-09';
export { default as PrefixItemsVisitor } from "./refractor/visitors/json-schema/PrefixItemsVisitor.mjs";
/**
 * JSON Schema 2020-12 specification elements.
 */
export { JSONSchemaElement, LinkDescriptionElement } from "./refractor/registration.mjs";