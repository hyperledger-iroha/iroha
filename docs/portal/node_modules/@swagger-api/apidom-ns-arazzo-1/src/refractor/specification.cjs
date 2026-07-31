"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _apidomNsJsonSchema = require("@swagger-api/apidom-ns-json-schema-2020-12");
var _index = _interopRequireDefault(require("./visitors/arazzo-1/index.cjs"));
var _ArazzoSpecVisitor = _interopRequireDefault(require("./visitors/arazzo-1/ArazzoSpecVisitor.cjs"));
var _index2 = _interopRequireDefault(require("./visitors/arazzo-1/info/index.cjs"));
var _VersionVisitor = _interopRequireDefault(require("./visitors/arazzo-1/info/VersionVisitor.cjs"));
var _index3 = _interopRequireDefault(require("./visitors/arazzo-1/source-description/index.cjs"));
var _UrlVisitor = _interopRequireDefault(require("./visitors/arazzo-1/source-description/UrlVisitor.cjs"));
var _index4 = _interopRequireDefault(require("./visitors/arazzo-1/workflow/index.cjs"));
var _StepsVisitor = _interopRequireDefault(require("./visitors/arazzo-1/workflow/StepsVisitor.cjs"));
var _OutputsVisitor = _interopRequireDefault(require("./visitors/arazzo-1/workflow/OutputsVisitor.cjs"));
var _index5 = _interopRequireDefault(require("./visitors/arazzo-1/step/index.cjs"));
var _OutputsVisitor2 = _interopRequireDefault(require("./visitors/arazzo-1/step/OutputsVisitor.cjs"));
var _ParametersVisitor = _interopRequireDefault(require("./visitors/arazzo-1/step/ParametersVisitor.cjs"));
var _DependsOnVisitor = _interopRequireDefault(require("./visitors/arazzo-1/step/DependsOnVisitor.cjs"));
var _SuccessCriteriaVisitor = _interopRequireDefault(require("./visitors/arazzo-1/step/SuccessCriteriaVisitor.cjs"));
var _OnSuccessVisitor = _interopRequireDefault(require("./visitors/arazzo-1/step/OnSuccessVisitor.cjs"));
var _OnFailureVisitor = _interopRequireDefault(require("./visitors/arazzo-1/step/OnFailureVisitor.cjs"));
var _index6 = _interopRequireDefault(require("./visitors/arazzo-1/parameter/index.cjs"));
var _SourceDescriptionsVisitor = _interopRequireDefault(require("./visitors/arazzo-1/SourceDescriptionsVisitor.cjs"));
var _WorkflowsVisitor = _interopRequireDefault(require("./visitors/arazzo-1/WorkflowsVisitor.cjs"));
var _index7 = _interopRequireDefault(require("./visitors/arazzo-1/success-action/index.cjs"));
var _SuccessActionCriteriaVisitor = _interopRequireDefault(require("./visitors/arazzo-1/SuccessActionCriteriaVisitor.cjs"));
var _index8 = _interopRequireDefault(require("./visitors/arazzo-1/failure-action/index.cjs"));
var _FailureActionCriteriaVisitor = _interopRequireDefault(require("./visitors/arazzo-1/FailureActionCriteriaVisitor.cjs"));
var _index9 = _interopRequireDefault(require("./visitors/arazzo-1/components/index.cjs"));
var _InputsVisitor = _interopRequireDefault(require("./visitors/arazzo-1/components/InputsVisitor.cjs"));
var _ParametersVisitor2 = _interopRequireDefault(require("./visitors/arazzo-1/components/ParametersVisitor.cjs"));
var _index0 = _interopRequireDefault(require("./visitors/arazzo-1/criterion/index.cjs"));
var _index1 = _interopRequireDefault(require("./visitors/arazzo-1/reference/index.cjs"));
var _$RefVisitor = _interopRequireDefault(require("./visitors/arazzo-1/reference/$RefVisitor.cjs"));
var _SpecificationExtensionVisitor = _interopRequireDefault(require("./visitors/SpecificationExtensionVisitor.cjs"));
var _FallbackVisitor = _interopRequireDefault(require("./visitors/FallbackVisitor.cjs"));
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
} = _apidomNsJsonSchema.specificationObj.visitors.document.objects;

/**
 * @public
 */
const specification = {
  visitors: {
    value: _FallbackVisitor.default,
    document: {
      objects: {
        ArazzoSpecification: {
          $visitor: _index.default,
          fixedFields: {
            arazzo: _ArazzoSpecVisitor.default,
            info: {
              $ref: '#/visitors/document/objects/Info'
            },
            sourceDescriptions: _SourceDescriptionsVisitor.default,
            workflows: _WorkflowsVisitor.default,
            components: {
              $ref: '#/visitors/document/objects/Components'
            }
          }
        },
        Info: {
          $visitor: _index2.default,
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
            version: _VersionVisitor.default
          }
        },
        SourceDescription: {
          $visitor: _index3.default,
          fixedFields: {
            name: {
              $ref: '#/visitors/value'
            },
            url: _UrlVisitor.default,
            type: {
              $ref: '#/visitors/value'
            }
          }
        },
        Workflow: {
          $visitor: _index4.default,
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
            steps: _StepsVisitor.default,
            outputs: _OutputsVisitor.default
          }
        },
        Step: {
          $visitor: _index5.default,
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
            parameters: _ParametersVisitor.default,
            dependsOn: _DependsOnVisitor.default,
            successCriteria: _SuccessCriteriaVisitor.default,
            onSuccess: _OnSuccessVisitor.default,
            onFailure: _OnFailureVisitor.default,
            outputs: _OutputsVisitor2.default
          }
        },
        Parameter: {
          $visitor: _index6.default,
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
          $visitor: _index7.default,
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
            criteria: _SuccessActionCriteriaVisitor.default
          }
        },
        FailureAction: {
          $visitor: _index8.default,
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
            criteria: _FailureActionCriteriaVisitor.default
          }
        },
        Components: {
          $visitor: _index9.default,
          fixedFields: {
            inputs: _InputsVisitor.default,
            parameters: _ParametersVisitor2.default
          }
        },
        Criterion: {
          $visitor: _index0.default,
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
          $visitor: _index1.default,
          fixedFields: {
            $ref: _$RefVisitor.default,
            value: {
              $ref: '#/visitors/value'
            }
          }
        },
        JSONSchema: JSONSchemaVisitor
      },
      extension: {
        $visitor: _SpecificationExtensionVisitor.default
      }
    }
  }
};
var _default = exports.default = specification;