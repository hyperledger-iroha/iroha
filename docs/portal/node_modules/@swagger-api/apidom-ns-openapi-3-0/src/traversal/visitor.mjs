import { keyMap as keyMapBase, isElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
export const getNodeType = element => {
  if (!isElement(element)) {
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

export const keyMap = {
  CallbackElement: ['content'],
  ComponentsElement: ['content'],
  ContactElement: ['content'],
  DiscriminatorElement: ['content'],
  Encoding: ['content'],
  Example: ['content'],
  ExternalDocumentationElement: ['content'],
  HeaderElement: ['content'],
  InfoElement: ['content'],
  LicenseElement: ['content'],
  MediaTypeElement: ['content'],
  OAuthFlowElement: ['content'],
  OAuthFlowsElement: ['content'],
  OpenApi3_0Element: ['content'],
  OperationElement: ['content'],
  ParameterElement: ['content'],
  PathItemElement: ['content'],
  PathsElement: ['content'],
  ReferenceElement: ['content'],
  RequestBodyElement: ['content'],
  ResponseElement: ['content'],
  ResponsesElement: ['content'],
  SchemaElement: ['content'],
  SecurityRequirementElement: ['content'],
  SecuritySchemeElement: ['content'],
  ServerElement: ['content'],
  ServerVariableElement: ['content'],
  TagElement: ['content'],
  ...keyMapBase
};