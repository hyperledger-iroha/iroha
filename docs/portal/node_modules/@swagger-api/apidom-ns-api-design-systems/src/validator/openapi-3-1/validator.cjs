"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
var _apidomError = require("@swagger-api/apidom-error");
var _selector = _interopRequireDefault(require("./selector.cjs"));
var _accessor = _interopRequireDefault(require("./accessor.cjs"));
var _requirementLevel = require("./requirement-level.cjs");
const makeMessage = (value, requirementLevel, standardIdentifier) => {
  const primitiveValue = (0, _apidomCore.toValue)(value);
  const primitiveStandardIdentifier = JSON.stringify((0, _apidomCore.toValue)(standardIdentifier));
  if ((0, _apidomCore.toValue)(requirementLevel) === 'may') {
    return `"${primitiveValue}" not allowed for subject ${primitiveStandardIdentifier}`;
  }
  throw new _apidomError.NotImplementedError('[Requirement levels] other than "may" are not yet implemented.');
};

// @ts-ignore
const makeAnnotation = (message, value, level, standardIdentifier) => {
  const annotation = new _apidomCore.AnnotationElement(message);
  annotation.classes.push(level);
  annotation.attributes.set('value', (0, _apidomCore.cloneDeep)(value));
  annotation.attributes.set('standardIdentifier', (0, _apidomCore.cloneDeep)(standardIdentifier));
  return annotation;
};
const validateValue = (value, requirement) => {
  const annotations = [];
  const {
    subject
  } = requirement;
  if (typeof requirement.values === 'undefined') return annotations;
  if ((0, _apidomCore.toValue)(requirement.level) === 'may') {
    const isValid = (0, _requirementLevel.may)((0, _apidomCore.toValue)(value), (0, _apidomCore.toValue)(requirement.values));
    if (!isValid) {
      const message = makeMessage(value, requirement.level, subject);
      const annotation = makeAnnotation(message, value, 'error', subject);
      annotations.push(annotation);
    }
  }
  return annotations;
};
const validateRequirement = (requirement, selected) => {
  const {
    subject
  } = requirement;
  const values = (0, _accessor.default)(selected, subject);
  const annotations = [];
  values.forEach(value => {
    annotations.push(...validateValue(value, requirement));
  });
  return annotations;
};
const validateScenario = (scenario, openApiElement) => {
  const annotations = [];
  const {
    when
  } = scenario;
  const selected = (0, _selector.default)(openApiElement, when);
  const {
    then: requirements
  } = scenario;
  if (typeof requirements === 'undefined') return annotations;
  selected.forEach(item => {
    // @ts-ignore
    requirements.forEach(requirement => {
      annotations.push(...validateRequirement(requirement, item));
    });
  });
  return annotations;
};

/**
 * @public
 */
const validate = (mainElement, openApiElement) => {
  const {
    scenarios
  } = mainElement;
  const annotations = [];
  if (typeof scenarios === 'undefined' || !(0, _apidomCore.isArrayElement)(scenarios)) return [];

  // @ts-ignore
  scenarios.forEach(scenario => {
    annotations.push(...validateScenario(scenario, openApiElement));
  });
  return annotations;
};
var _default = exports.default = validate;