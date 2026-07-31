"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _FallbackVisitor = _interopRequireDefault(require("./visitors/FallbackVisitor.cjs"));
var _index = _interopRequireDefault(require("./visitors/api-design-systems/requirement-level/index.cjs"));
var _index2 = _interopRequireDefault(require("./visitors/api-design-systems/standard-identifier/index.cjs"));
var _index3 = _interopRequireDefault(require("./visitors/api-design-systems/requirement/index.cjs"));
var _index4 = _interopRequireDefault(require("./visitors/api-design-systems/scenario/index.cjs"));
var _ThenVisitor = _interopRequireDefault(require("./visitors/api-design-systems/scenario/ThenVisitor.cjs"));
var _index5 = _interopRequireDefault(require("./visitors/api-design-systems/standard/index.cjs"));
var _index6 = _interopRequireDefault(require("./visitors/api-design-systems/principle/index.cjs"));
var _index7 = _interopRequireDefault(require("./visitors/api-design-systems/info/index.cjs"));
var _index8 = _interopRequireDefault(require("./visitors/api-design-systems/main/index.cjs"));
var _PrinciplesVisitor = _interopRequireDefault(require("./visitors/api-design-systems/main/PrinciplesVisitor.cjs"));
var _StandardsVisitor = _interopRequireDefault(require("./visitors/api-design-systems/main/StandardsVisitor.cjs"));
var _ScenariosVisitor = _interopRequireDefault(require("./visitors/api-design-systems/main/ScenariosVisitor.cjs"));
/**
 * API Design Systems 2021-05-07 specification elements.
 */

/**
 * Specification object allows us to have complete control over visitors
 * when traversing the ApiDOM.
 * Specification also allows us to create amended refractors from
 * existing ones by manipulating it.
 *
 * Note: Specification object allows to use absolute internal JSON pointers.
 * @public
 */
const specification = {
  visitors: {
    value: _FallbackVisitor.default,
    document: {
      objects: {
        /**
         * API Design Systems 2021-05-07 specification elements.
         */
        Main: {
          $visitor: _index8.default,
          fixedFields: {
            version: {
              $ref: '#/visitors/value'
            },
            info: {
              $ref: '#/visitors/document/objects/Info'
            },
            principles: _PrinciplesVisitor.default,
            standards: _StandardsVisitor.default,
            scenarios: _ScenariosVisitor.default
          }
        },
        Info: {
          $visitor: _index7.default,
          fixedFields: {
            title: {
              $ref: '#/visitors/value'
            },
            description: {
              $ref: '#/visitors/value'
            }
          }
        },
        Principle: {
          $visitor: _index6.default,
          fixedFields: {
            name: {
              $ref: '#/visitors/value'
            },
            description: {
              $ref: '#/visitors/value'
            },
            iri: {
              $ref: '#/visitors/value'
            },
            level: {
              $ref: '#/visitors/document/objects/RequirementLevel'
            }
          }
        },
        Standard: {
          $visitor: _index5.default,
          fixedFields: {
            name: {
              $ref: '#/visitors/value'
            },
            description: {
              $ref: '#/visitors/value'
            },
            level: {
              $ref: '#/visitors/document/objects/RequirementLevel'
            },
            iri: {
              $ref: '#/visitors/value'
            }
          }
        },
        Scenario: {
          $visitor: _index4.default,
          fixedFields: {
            description: {
              $ref: '#/visitors/value'
            },
            when: {
              $ref: '#/visitors/document/objects/StandardIdentifier'
            },
            then: _ThenVisitor.default
          }
        },
        Requirement: {
          $visitor: _index3.default,
          fixedFields: {
            subject: {
              $ref: '#/visitors/document/objects/StandardIdentifier'
            },
            level: {
              $ref: '#/visitors/document/objects/RequirementLevel'
            },
            values: {
              $ref: '#/visitors/value'
            },
            follows: {
              $ref: '#/visitors/value'
            }
          }
        },
        StandardIdentifier: {
          $visitor: _index2.default
        },
        RequirementLevel: {
          $visitor: _index.default
        }
      }
    }
  }
};
var _default = exports.default = specification;