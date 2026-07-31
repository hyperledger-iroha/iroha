import { createPredicate } from '@swagger-api/apidom-core';
import MainElement from "./elements/Main.mjs";
import InfoElement from "./elements/Info.mjs";
import PrincipleElement from "./elements/Principle.mjs";
import RequirementElement from "./elements/Requirement.mjs";
import RequirementLevelElement from "./elements/RequirementLevel.mjs";
import ScenarioElement from "./elements/Scenario.mjs";
import StandardElement from "./elements/Standard.mjs";
import StandardIdentifierElement from "./elements/StandardIdentifier.mjs";
/**
 * @public
 */
export const isMainElement = createPredicate(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof MainElement || hasBasicElementProps(element) && isElementType('main', element) && primitiveEq('object', element);
});

/**
 * @public
 */
export const isInfoElement = createPredicate(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof InfoElement || hasBasicElementProps(element) && isElementType('info', element) && primitiveEq('object', element);
});

/**
 * @public
 */
export const isPrincipleElement = createPredicate(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof PrincipleElement || hasBasicElementProps(element) && isElementType('principle', element) && primitiveEq('object', element);
});

/**
 * @public
 */
export const isRequirementElement = createPredicate(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof RequirementElement || hasBasicElementProps(element) && isElementType('requirement', element) && primitiveEq('object', element);
});

/**
 * @public
 */
export const isRequirementLevelElement = createPredicate(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof RequirementLevelElement || hasBasicElementProps(element) && isElementType('requirementLevel', element) && primitiveEq('string', element);
});

/**
 * @public
 */
export const isScenarioElement = createPredicate(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof ScenarioElement || hasBasicElementProps(element) && isElementType('scenario', element) && primitiveEq('object', element);
});

/**
 * @public
 */
export const isStandardElement = createPredicate(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof StandardElement || hasBasicElementProps(element) && isElementType('standard', element) && primitiveEq('object', element);
});

/**
 * @public
 */
export const isStandardIdentifierElement = createPredicate(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof StandardIdentifierElement || hasBasicElementProps(element) && isElementType('standardIdentifier', element) && primitiveEq('array', element);
});