import { ArrayElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class WorkflowSteps extends ArrayElement {
  static primaryClass = 'workflow-steps';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(WorkflowSteps.primaryClass);
  }
}
export default WorkflowSteps;