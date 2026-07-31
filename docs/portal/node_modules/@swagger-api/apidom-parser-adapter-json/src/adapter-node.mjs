import lexicalAnalysis from "./lexical-analysis/node.mjs";
import syntacticAnalysisDirect from "./syntactic-analysis/direct/index.mjs";
import syntacticAnalysisIndirect from "./syntactic-analysis/indirect/index.mjs";
import { detectionRegExp } from "./adapter.mjs";
export { mediaTypes, namespace } from "./adapter.mjs";
export { detectionRegExp };
export { lexicalAnalysis, syntacticAnalysisDirect as syntacticAnalysis, syntacticAnalysisDirect, syntacticAnalysisIndirect };

/**
 * @public
 */
export const detect = async source => {
  if (!detectionRegExp.test(source)) {
    return false;
  }
  try {
    const cst = await lexicalAnalysis(source);
    return cst.rootNode.type !== 'ERROR';
  } catch {
    return false;
  }
};

/**
 * @public
 */

/**
 * @public
 */

/**
 * @public
 */
export const parse = async (source, {
  sourceMap = false,
  syntacticAnalysis = 'direct'
} = {}) => {
  const cst = await lexicalAnalysis(source);
  let apiDOM;
  if (syntacticAnalysis === 'indirect') {
    apiDOM = syntacticAnalysisIndirect(cst, {
      sourceMap
    });
  } else {
    apiDOM = syntacticAnalysisDirect(cst, {
      sourceMap
    });
  }
  return apiDOM;
};