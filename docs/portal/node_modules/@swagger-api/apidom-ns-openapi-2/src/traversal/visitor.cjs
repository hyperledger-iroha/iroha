"use strict";

exports.__esModule = true;
exports.keyMap = exports.getNodeType = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
const getNodeType = element => {
  if (!(0, _apidomCore.isElement)(element)) {
    return undefined;
  }
  return `${element.element.charAt(0).toUpperCase() + element.element.slice(1)}Element`;
};

/**
 * There are unfortunately two `LinkElement` types. One is from base namespace
 * and the other one if from this namespace. `LinkElement` from base namespace
 * is used extremely rarely, so it's almost always safe during traversing
 * to assume that `LinkElement` is element from this namespace.
 *
 * To be 100% sure that currently visiting `LinkElement` is from this namespace
 * use `isLinkElement` predicate from this namespace to assert for it.
 * @public
 */
exports.getNodeType = getNodeType;
const keyMap = exports.keyMap = {
  SwaggerElement: ['content'],
  InfoElement: ['content'],
  ContactElement: ['content'],
  LicenseElement: ['content'],
  PathsElement: ['content'],
  PathItemElement: ['content'],
  OperationElement: ['content'],
  ExternalDocumentationElement: ['content'],
  ParameterElement: ['content'],
  ItemsElement: ['content'],
  ExampleElement: ['content'],
  ResponsesElement: ['content'],
  ResponseElement: ['content'],
  HeadersElement: ['content'],
  HeaderElement: ['content'],
  TagElement: ['content'],
  ReferenceElement: ['content'],
  JSONReferenceElement: ['content'],
  SchemaElement: ['content'],
  XmlElement: ['content'],
  DefinitionsElement: ['content'],
  ParametersDefinitionsElement: ['content'],
  ResponsesDefinitionsElement: ['content'],
  SecurityDefinitionsElement: ['content'],
  SecuritySchemeElement: ['content'],
  ScopesElement: ['content'],
  SecurityRequirementElement: ['content'],
  ..._apidomCore.keyMap
};