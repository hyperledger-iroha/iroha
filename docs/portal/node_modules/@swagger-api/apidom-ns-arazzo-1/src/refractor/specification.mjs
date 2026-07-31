import { specificationObj as jsonSchemaSpecificationObj } from '@swagger-api/apidom-ns-json-schema-2020-12';
import ArazzoSpecificationVisitor from "./visitors/arazzo-1/index.mjs";
import ArazzoSpecVisitor from "./visitors/arazzo-1/ArazzoSpecVisitor.mjs";
import InfoVisitor from "./visitors/arazzo-1/info/index.mjs";
import InfoVersionVisitor from "./visitors/arazzo-1/info/VersionVisitor.mjs";
import SourceDescriptionVisitor from "./visitors/arazzo-1/source-description/index.mjs";
import SourceDescriptionUrlVisitor from "./visitors/arazzo-1/source-description/UrlVisitor.mjs";
import WorkflowVisitor from "./visitors/arazzo-1/workflow/index.mjs";
import WorkflowStepsVisitor from "./visitors/arazzo-1/workflow/StepsVisitor.mjs";
import workflowOutputsVisitor from "./visitors/arazzo-1/workflow/OutputsVisitor.mjs";
import StepVisitor from "./visitors/arazzo-1/step/index.mjs";
import StepOutputsVisitor from "./visitors/arazzo-1/step/OutputsVisitor.mjs";
import StepParametersVisitor from "./visitors/arazzo-1/step/ParametersVisitor.mjs";
import StepDependsOnVisitor from "./visitors/arazzo-1/step/DependsOnVisitor.mjs";
import StepSuccessCriteriaVisitor from "./visitors/arazzo-1/step/SuccessCriteriaVisitor.mjs";
import StepOnSuccessVisitor from "./visitors/arazzo-1/step/OnSuccessVisitor.mjs";
import StepOnFailureVisitor from "./visitors/arazzo-1/step/OnFailureVisitor.mjs";
import ParameterVisitor from "./visitors/arazzo-1/parameter/index.mjs";
import SourceDescriptionsVisitor from "./visitors/arazzo-1/SourceDescriptionsVisitor.mjs";
import WorkflowsVisitor from "./visitors/arazzo-1/WorkflowsVisitor.mjs";
import SuccessActionVisitor from "./visitors/arazzo-1/success-action/index.mjs";
import SuccessActionCriteriaVisitor from "./visitors/arazzo-1/SuccessActionCriteriaVisitor.mjs";
import FailureActionVisitor from "./visitors/arazzo-1/failure-action/index.mjs";
import FailureActionCriteriaVisitor from "./visitors/arazzo-1/FailureActionCriteriaVisitor.mjs";
import ComponentsVisitor from "./visitors/arazzo-1/components/index.mjs";
import ComponentsInputsVisitor from "./visitors/arazzo-1/components/InputsVisitor.mjs";
import ComponentsParametersVisitor from "./visitors/arazzo-1/components/ParametersVisitor.mjs";
import CriterionVisitor from "./visitors/arazzo-1/criterion/index.mjs";
import ReferenceVisitor from "./visitors/arazzo-1/reference/index.mjs";
import Reference$RefVisitor from "./visitors/arazzo-1/reference/$RefVisitor.mjs";
import SpecificationExtensionVisitor from "./visitors/SpecificationExtensionVisitor.mjs";
import FallbackVisitor from "./visitors/FallbackVisitor.mjs";
/**
 * Specification object allows us to have complete control over visitors
 * when traversing the ApiDOM.
 * Specification also allows us to create amended refractors from
 * existing ones by manipulating it.
 *
 * Note: Specification object allows to use absolute internal JSON pointers.
 */
const {
  JSONSchema: JSONSchemaVisitor
} = jsonSchemaSpecificationObj.visitors.document.objects;

/**
 * @public
 */
const specification = {
  visitors: {
    value: FallbackVisitor,
    document: {
      objects: {
        ArazzoSpecification: {
          $visitor: ArazzoSpecificationVisitor,
          fixedFields: {
            arazzo: ArazzoSpecVisitor,
            info: {
              $ref: '#/visitors/document/objects/Info'
            },
            sourceDescriptions: SourceDescriptionsVisitor,
            workflows: WorkflowsVisitor,
            components: {
              $ref: '#/visitors/document/objects/Components'
            }
          }
        },
        Info: {
          $visitor: InfoVisitor,
          fixedFields: {
            title: {
              $ref: '#/visitors/value'
            },
            summary: {
              $ref: '#/visitors/value'
            },
            description: {
              $ref: '#/visitors/value'
            },
            version: InfoVersionVisitor
          }
        },
        SourceDescription: {
          $visitor: SourceDescriptionVisitor,
          fixedFields: {
            name: {
              $ref: '#/visitors/value'
            },
            url: SourceDescriptionUrlVisitor,
            type: {
              $ref: '#/visitors/value'
            }
          }
        },
        Workflow: {
          $visitor: WorkflowVisitor,
          fixedFields: {
            workflowId: {
              $ref: '#/visitors/value'
            },
            summary: {
              $ref: '#/visitors/value'
            },
            description: {
              $ref: '#/visitors/value'
            },
            inputs: JSONSchemaVisitor,
            steps: WorkflowStepsVisitor,
            outputs: workflowOutputsVisitor
          }
        },
        Step: {
          $visitor: StepVisitor,
          fixedFields: {
            description: {
              $ref: '#/visitors/value'
            },
            stepId: {
              $ref: '#/visitors/value'
            },
            operationId: {
              $ref: '#/visitors/value'
            },
            operationRef: {
              $ref: '#/visitors/value'
            },
            workflowId: {
              $ref: '#/visitors/value'
            },
            parameters: StepParametersVisitor,
            dependsOn: StepDependsOnVisitor,
            successCriteria: StepSuccessCriteriaVisitor,
            onSuccess: StepOnSuccessVisitor,
            onFailure: StepOnFailureVisitor,
            outputs: StepOutputsVisitor
          }
        },
        Parameter: {
          $visitor: ParameterVisitor,
          fixedFields: {
            name: {
              $ref: '#/visitors/value'
            },
            in: {
              $ref: '#/visitors/value'
            },
            style: {
              $ref: '#/visitors/value'
            },
            target: {
              $ref: '#/visitors/value'
            },
            value: {
              $ref: '#/visitors/value'
            }
          }
        },
        SuccessAction: {
          $visitor: SuccessActionVisitor,
          fixedFields: {
            type: {
              $ref: '#/visitors/value'
            },
            workflowId: {
              $ref: '#/visitors/value'
            },
            stepId: {
              $ref: '#/visitors/value'
            },
            criteria: SuccessActionCriteriaVisitor
          }
        },
        FailureAction: {
          $visitor: FailureActionVisitor,
          fixedFields: {
            type: {
              $ref: '#/visitors/value'
            },
            workflowId: {
              $ref: '#/visitors/value'
            },
            stepId: {
              $ref: '#/visitors/value'
            },
            retryAfter: {
              $ref: '#/visitors/value'
            },
            retryLimit: {
              $ref: '#/visitors/value'
            },
            criteria: FailureActionCriteriaVisitor
          }
        },
        Components: {
          $visitor: ComponentsVisitor,
          fixedFields: {
            inputs: ComponentsInputsVisitor,
            parameters: ComponentsParametersVisitor
          }
        },
        Criterion: {
          $visitor: CriterionVisitor,
          fixedFields: {
            context: {
              $ref: '#/visitors/value'
            },
            condition: {
              $ref: '#/visitors/value'
            },
            type: {
              $ref: '#/visitors/value'
            }
          }
        },
        Reference: {
          $visitor: ReferenceVisitor,
          fixedFields: {
            $ref: Reference$RefVisitor,
            value: {
              $ref: '#/visitors/value'
            }
          }
        },
        JSONSchema: JSONSchemaVisitor
      },
      extension: {
        $visitor: SpecificationExtensionVisitor
      }
    }
  }
};
export default specification;