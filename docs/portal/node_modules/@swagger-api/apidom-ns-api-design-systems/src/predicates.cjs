"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.isStandardIdentifierElement = exports.isStandardElement = exports.isScenarioElement = exports.isRequirementLevelElement = exports.isRequirementElement = exports.isPrincipleElement = exports.isMainElement = exports.isInfoElement = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
var _Main = _interopRequireDefault(require("./elements/Main.cjs"));
var _Info = _interopRequireDefault(require("./elements/Info.cjs"));
var _Principle = _interopRequireDefault(require("./elements/Principle.cjs"));
var _Requirement = _interopRequireDefault(require("./elements/Requirement.cjs"));
var _RequirementLevel = _interopRequireDefault(require("./elements/RequirementLevel.cjs"));
var _Scenario = _interopRequireDefault(require("./elements/Scenario.cjs"));
var _Standard = _interopRequireDefault(require("./elements/Standard.cjs"));
var _StandardIdentifier = _interopRequireDefault(require("./elements/StandardIdentifier.cjs"));
/**
 * @public
 */
const isMainElement = exports.isMainElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _Main.default || hasBasicElementProps(element) && isElementType('main', element) && primitiveEq('object', element);
});

/**
 * @public
 */
const isInfoElement = exports.isInfoElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _Info.default || hasBasicElementProps(element) && isElementType('info', element) && primitiveEq('object', element);
});

/**
 * @public
 */
const isPrincipleElement = exports.isPrincipleElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _Principle.default || hasBasicElementProps(element) && isElementType('principle', element) && primitiveEq('object', element);
});

/**
 * @public
 */
const isRequirementElement = exports.isRequirementElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _Requirement.default || hasBasicElementProps(element) && isElementType('requirement', element) && primitiveEq('object', element);
});

/**
 * @public
 */
const isRequirementLevelElement = exports.isRequirementLevelElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _RequirementLevel.default || hasBasicElementProps(element) && isElementType('requirementLevel', element) && primitiveEq('string', element);
});

/**
 * @public
 */
const isScenarioElement = exports.isScenarioElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _Scenario.default || hasBasicElementProps(element) && isElementType('scenario', element) && primitiveEq('object', element);
});

/**
 * @public
 */
const isStandardElement = exports.isStandardElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _Standard.default || hasBasicElementProps(element) && isElementType('standard', element) && primitiveEq('object', element);
});

/**
 * @public
 */
const isStandardIdentifierElement = exports.isStandardIdentifierElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _StandardIdentifier.default || hasBasicElementProps(element) && isElementType('standardIdentifier', element) && primitiveEq('array', element);
});