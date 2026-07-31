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
 * Arazzo Specification 1.0.1
 * @public
 */

export const keyMap = {
  ArazzoSpecification1Element: ['content'],
  InfoElement: ['content'],
  SourceDescriptionElement: ['content'],
  WorkflowElement: ['content'],
  StepElement: ['content'],
  ParameterElement: ['content'],
  SuccessActionElement: ['content'],
  FailureActionElement: ['content'],
  ComponentsElement: ['content'],
  CriterionElement: ['content'],
  ReferenceElement: ['content'],
  JSONSchema202012Element: ['content'],
  ...keyMapBase
};