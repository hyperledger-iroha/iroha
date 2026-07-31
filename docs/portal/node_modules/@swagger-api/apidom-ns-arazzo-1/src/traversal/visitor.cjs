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
 * Arazzo Specification 1.0.1
 * @public
 */
exports.getNodeType = getNodeType;
const keyMap = exports.keyMap = {
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
  ..._apidomCore.keyMap
};