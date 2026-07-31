import { JSONSchemaElement } from '@swagger-api/apidom-ns-json-schema-2020-12';
import ArazzoSpecification1Element from "./elements/ArazzoSpecification1.mjs";
import ArazzoSpecElement from "./elements/ArazzoSpec.mjs";
import InfoElement from "./elements/Info.mjs";
import SourceDescriptionElement from "./elements/SourceDescription.mjs";
import WorkflowElement from "./elements/Workflow.mjs";
import StepElement from "./elements/Step.mjs";
import ParameterElement from "./elements/Parameter.mjs";
import SuccessActionElement from "./elements/SuccessAction.mjs";
import FailureActionElement from "./elements/FailureAction.mjs";
import ComponentsElement from "./elements/Components.mjs";
import CriterionElement from "./elements/Criterion.mjs";
import ReferenceElement from "./elements/Reference.mjs";
/**
 * @public
 */
const arazzo1 = {
  namespace: options => {
    const {
      base
    } = options;
    base.register('arazzoSpecification1', ArazzoSpecification1Element);
    base.register('arazzoSpec', ArazzoSpecElement);
    base.register('info', InfoElement);
    base.register('sourceDescription', SourceDescriptionElement);
    base.register('workflow', WorkflowElement);
    base.register('step', StepElement);
    base.register('parameter', ParameterElement);
    base.register('successAction', SuccessActionElement);
    base.register('failureAction', FailureActionElement);
    base.register('components', ComponentsElement);
    base.register('criterion', CriterionElement);
    base.register('reference', ReferenceElement);
    base.register('jSONSchema202012', JSONSchemaElement);
    return base;
  }
};
export default arazzo1;