import { AnnotationElement, isArrayElement, toValue, cloneDeep } from '@swagger-api/apidom-core';
import { NotImplementedError } from '@swagger-api/apidom-error';
import select from "./selector.mjs";
import access from "./accessor.mjs";
import { may } from "./requirement-level.mjs";
const makeMessage = (value, requirementLevel, standardIdentifier) => {
  const primitiveValue = toValue(value);
  const primitiveStandardIdentifier = JSON.stringify(toValue(standardIdentifier));
  if (toValue(requirementLevel) === 'may') {
    return `"${primitiveValue}" not allowed for subject ${primitiveStandardIdentifier}`;
  }
  throw new NotImplementedError('[Requirement levels] other than "may" are not yet implemented.');
};

// @ts-ignore
const makeAnnotation = (message, value, level, standardIdentifier) => {
  const annotation = new AnnotationElement(message);
  annotation.classes.push(level);
  annotation.attributes.set('value', cloneDeep(value));
  annotation.attributes.set('standardIdentifier', cloneDeep(standardIdentifier));
  return annotation;
};
const validateValue = (value, requirement) => {
  const annotations = [];
  const {
    subject
  } = requirement;
  if (typeof requirement.values === 'undefined') return annotations;
  if (toValue(requirement.level) === 'may') {
    const isValid = may(toValue(value), toValue(requirement.values));
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
  const values = access(selected, subject);
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
  const selected = select(openApiElement, when);
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
  if (typeof scenarios === 'undefined' || !isArrayElement(scenarios)) return [];

  // @ts-ignore
  scenarios.forEach(scenario => {
    annotations.push(...validateScenario(scenario, openApiElement));
  });
  return annotations;
};
export default validate;