"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.isWorkflowsElement = exports.isWorkflowStepsElement = exports.isWorkflowOutputsElement = exports.isWorkflowElement = exports.isSuccessActionElement = exports.isSuccessActionCriteriaElement = exports.isStepSuccessCriteriaElement = exports.isStepParametersElement = exports.isStepOutputsElement = exports.isStepOnSuccessElement = exports.isStepOnFailureElement = exports.isStepElement = exports.isStepDependsOnElement = exports.isSourceDescriptionsElement = exports.isSourceDescriptionElement = exports.isReferenceElement = exports.isParameterElement = exports.isInfoElement = exports.isFailureActionElement = exports.isFailureActionCriteriaElement = exports.isCriterionElement = exports.isComponentsElement = exports.isArazzoSpecification1Element = exports.isArazzoSpecElement = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
var _apidomNsJsonSchema = require("@swagger-api/apidom-ns-json-schema-2020-12");
exports.isJSONSchemaElement = _apidomNsJsonSchema.isJSONSchemaElement;
var _ArazzoSpecification = _interopRequireDefault(require("./elements/ArazzoSpecification1.cjs"));
var _ArazzoSpec = _interopRequireDefault(require("./elements/ArazzoSpec.cjs"));
var _Info = _interopRequireDefault(require("./elements/Info.cjs"));
var _SourceDescription = _interopRequireDefault(require("./elements/SourceDescription.cjs"));
var _Workflow = _interopRequireDefault(require("./elements/Workflow.cjs"));
var _Step = _interopRequireDefault(require("./elements/Step.cjs"));
var _Parameter = _interopRequireDefault(require("./elements/Parameter.cjs"));
var _SuccessAction = _interopRequireDefault(require("./elements/SuccessAction.cjs"));
var _FailureAction = _interopRequireDefault(require("./elements/FailureAction.cjs"));
var _Components = _interopRequireDefault(require("./elements/Components.cjs"));
var _Criterion = _interopRequireDefault(require("./elements/Criterion.cjs"));
var _Reference = _interopRequireDefault(require("./elements/Reference.cjs"));
var _Workflows = _interopRequireDefault(require("./elements/nces/Workflows.cjs"));
var _SourceDescriptions = _interopRequireDefault(require("./elements/nces/SourceDescriptions.cjs"));
var _WorkflowSteps = _interopRequireDefault(require("./elements/nces/WorkflowSteps.cjs"));
var _WorkflowOutputs = _interopRequireDefault(require("./elements/nces/WorkflowOutputs.cjs"));
var _StepParameters = _interopRequireDefault(require("./elements/nces/StepParameters.cjs"));
var _StepDependsOn = _interopRequireDefault(require("./elements/nces/StepDependsOn.cjs"));
var _StepSuccessCriteria = _interopRequireDefault(require("./elements/nces/StepSuccessCriteria.cjs"));
var _StepOnSuccess = _interopRequireDefault(require("./elements/nces/StepOnSuccess.cjs"));
var _StepOnFailure = _interopRequireDefault(require("./elements/nces/StepOnFailure.cjs"));
var _StepOutputs = _interopRequireDefault(require("./elements/nces/StepOutputs.cjs"));
var _SuccessActionCriteria = _interopRequireDefault(require("./elements/nces/SuccessActionCriteria.cjs"));
var _FailureActionCriteria = _interopRequireDefault(require("./elements/nces/FailureActionCriteria.cjs"));
// NCE types

/**
 * @public
 */
const isArazzoSpecElement = exports.isArazzoSpecElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _ArazzoSpec.default || hasBasicElementProps(element) && isElementType('arazzoSpec', element) && primitiveEq('string', element);
});

/**
 * @public
 */
const isArazzoSpecification1Element = exports.isArazzoSpecification1Element = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq,
  hasClass
}) => {
  return element => element instanceof _ArazzoSpecification.default || hasBasicElementProps(element) && isElementType('arazzoSpecification1', element) && primitiveEq('object', element) && hasClass('api', element) && hasClass('arazzo', element);
});

/**
 * @public
 */
const isInfoElement = exports.isInfoElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq,
  hasClass
}) => {
  return element => element instanceof _Info.default || hasBasicElementProps(element) && isElementType('info', element) && primitiveEq('object', element) && hasClass('info', element);
});

/**
 * @public
 */
const isSourceDescriptionElement = exports.isSourceDescriptionElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _SourceDescription.default || hasBasicElementProps(element) && isElementType('sourceDescription', element) && primitiveEq('object', element);
});

/**
 * @public
 */
const isSourceDescriptionsElement = exports.isSourceDescriptionsElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq,
  hasClass
}) => {
  return element => element instanceof _SourceDescriptions.default || hasBasicElementProps(element) && isElementType('sourceDescriptions', element) && primitiveEq('array', element) && hasClass('sourceDescriptions', element);
});

/**
 * @public
 */
const isWorkflowsElement = exports.isWorkflowsElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq,
  hasClass
}) => {
  return element => element instanceof _Workflows.default || hasBasicElementProps(element) && isElementType('workflows', element) && primitiveEq('array', element) && hasClass('workflows', element);
});

/**
 * @public
 */
const isWorkflowStepsElement = exports.isWorkflowStepsElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq,
  hasClass
}) => {
  return element => element instanceof _WorkflowSteps.default || hasBasicElementProps(element) && isElementType('array', element) && primitiveEq('array', element) && hasClass('workflow-steps', element);
});

/**
 * @public
 */
const isWorkflowOutputsElement = exports.isWorkflowOutputsElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq,
  hasClass
}) => {
  return element => element instanceof _WorkflowOutputs.default || hasBasicElementProps(element) && isElementType('array', element) && primitiveEq('array', element) && hasClass('workflow-outputs', element);
});

/**
 * @public
 */
const isWorkflowElement = exports.isWorkflowElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _Workflow.default || hasBasicElementProps(element) && isElementType('workflow', element) && primitiveEq('object', element);
});

/**
 * @public
 */
const isStepOnSuccessElement = exports.isStepOnSuccessElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq,
  hasClass
}) => {
  return element => element instanceof _StepOnSuccess.default || hasBasicElementProps(element) && isElementType('array', element) && primitiveEq('array', element) && hasClass('step-on-success', element);
});

/**
 * @public
 */
const isStepOnFailureElement = exports.isStepOnFailureElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq,
  hasClass
}) => {
  return element => element instanceof _StepOnFailure.default || hasBasicElementProps(element) && isElementType('array', element) && primitiveEq('array', element) && hasClass('step-on-failure', element);
});

/**
 * @public
 */
const isStepOutputsElement = exports.isStepOutputsElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq,
  hasClass
}) => {
  return element => element instanceof _StepOutputs.default || hasBasicElementProps(element) && isElementType('array', element) && primitiveEq('array', element) && hasClass('step-outputs', element);
});

/**
 * @public
 */
const isStepElement = exports.isStepElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _Step.default || hasBasicElementProps(element) && isElementType('step', element) && primitiveEq('object', element);
});

/**
 * @public
 */
const isStepParametersElement = exports.isStepParametersElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq,
  hasClass
}) => {
  return element => element instanceof _StepParameters.default || hasBasicElementProps(element) && isElementType('array', element) && primitiveEq('array', element) && hasClass('step-parameters', element);
});

/**
 * @public
 */
const isStepDependsOnElement = exports.isStepDependsOnElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq,
  hasClass
}) => {
  return element => element instanceof _StepDependsOn.default || hasBasicElementProps(element) && isElementType('array', element) && primitiveEq('array', element) && hasClass('step-depends-on', element);
});

/**
 * @public
 */
const isStepSuccessCriteriaElement = exports.isStepSuccessCriteriaElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq,
  hasClass
}) => {
  return element => element instanceof _StepSuccessCriteria.default || hasBasicElementProps(element) && isElementType('array', element) && primitiveEq('array', element) && hasClass('step-success-criteria', element) && hasClass('criteria', element);
});

/**
 * @public
 */
const isParameterElement = exports.isParameterElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _Parameter.default || hasBasicElementProps(element) && isElementType('parameter', element) && primitiveEq('object', element);
});

/**
 * @public
 */
const isSuccessActionElement = exports.isSuccessActionElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _SuccessAction.default || hasBasicElementProps(element) && isElementType('successAction', element) && primitiveEq('object', element);
});

/**
 * @public
 */
const isComponentsElement = exports.isComponentsElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _Components.default || hasBasicElementProps(element) && isElementType('components', element) && primitiveEq('object', element);
});

/**
 * @public
 */
const isCriterionElement = exports.isCriterionElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _Criterion.default || hasBasicElementProps(element) && isElementType('criterion', element) && primitiveEq('object', element);
});

/**
 * @public
 */
const isReferenceElement = exports.isReferenceElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _Reference.default || hasBasicElementProps(element) && isElementType('reference', element) && primitiveEq('object', element);
});

/**
 * @public
 */
const isSuccessActionCriteriaElement = exports.isSuccessActionCriteriaElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq,
  hasClass
}) => {
  return element => element instanceof _SuccessActionCriteria.default || hasBasicElementProps(element) && isElementType('array', element) && primitiveEq('array', element) && hasClass('success-action-criteria', element) && hasClass('criteria', element);
});

/**
 * @public
 */
const isFailureActionElement = exports.isFailureActionElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq
}) => {
  return element => element instanceof _FailureAction.default || hasBasicElementProps(element) && isElementType('failureAction', element) && primitiveEq('object', element);
});

/**
 * @public
 */
const isFailureActionCriteriaElement = exports.isFailureActionCriteriaElement = (0, _apidomCore.createPredicate)(({
  hasBasicElementProps,
  isElementType,
  primitiveEq,
  hasClass
}) => {
  return element => element instanceof _FailureActionCriteria.default || hasBasicElementProps(element) && isElementType('array', element) && primitiveEq('array', element) && hasClass('failure-action-criteria', element) && hasClass('criteria', element);
});