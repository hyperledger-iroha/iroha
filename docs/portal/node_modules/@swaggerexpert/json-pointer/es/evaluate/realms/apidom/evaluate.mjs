import baseEvaluate from "../../../evaluate/index.mjs";
import ApiDOMEvaluationRealm from "./realm.mjs";
const evaluate = (value, jsonPointer, options = {}) => {
  return baseEvaluate(value, jsonPointer, {
    ...options,
    realm: new ApiDOMEvaluationRealm()
  });
};
export default evaluate;