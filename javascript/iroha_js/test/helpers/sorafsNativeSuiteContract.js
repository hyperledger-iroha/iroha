// Static test contracts only; never import or execute an SDK implementation.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import ts from "typescript";

const printer = ts.createPrinter({ removeComments: true });
export function parseSuiteSource(source) {
  const parsed = ts.createSourceFile("suite.js", source, ts.ScriptTarget.Latest, true, ts.ScriptKind.JS);
  assert.equal(parsed.parseDiagnostics.length, 0, "suite syntax must parse without recovery");
  return parsed;
}
export function printed(node, source) {
  return printer.printNode(ts.EmitHint.Unspecified, node, source);
}
export function astDigest(nodes, source) {
  return createHash("sha256").update(nodes.map((node) => printed(node, source)).join("\n")).digest("hex");
}
export function descendantNodes(root, predicate) {
  const output = [];
  function visit(node) {
    if (predicate(node)) output.push(node);
    ts.forEachChild(node, visit);
  }
  visit(root);
  return output;
}
export function assertions(source) {
  return descendantNodes(source, (node) => ts.isCallExpression(node) && (
    ts.isIdentifier(node.expression) && node.expression.text === "assert"
    || ts.isPropertyAccessExpression(node.expression)
      && ts.isIdentifier(node.expression.expression) && node.expression.expression.text === "assert"
  ));
}
export function inspectRegistration(sourceText, name, originalStatementCount) {
  const source = parseSuiteSource(sourceText);
  const imports = source.statements.filter(ts.isImportDeclaration);
  const declarations = source.statements.filter((node) => !ts.isImportDeclaration(node));
  assert.equal(declarations.length, 1, "shared module has only its registration declaration");
  const registration = declarations[0];
  assert.ok(ts.isFunctionDeclaration(registration));
  assert.equal(registration.name?.text, name);
  assert.deepEqual(registration.parameters.map((node) => node.name.getText(source)), ["context"]);
  assert.ok(registration.modifiers?.some((node) => node.kind === ts.SyntaxKind.ExportKeyword));
  for (const declaration of imports) {
    const specifier = declaration.moduleSpecifier.text;
    assert.ok(specifier.startsWith("node:") || specifier === "../helpers/nativeRequirements.js",
      `shared suite cannot import SDK or eager native implementation: ${specifier}`);
  }
  const statements = [...registration.body.statements];
  assert.ok(originalStatementCount > 0 && originalStatementCount <= statements.length);
  const body = statements.slice(-originalStatementCount);
  const prefix = statements.slice(0, -originalStatementCount);
  const cases = [];
  for (const statement of body) {
    if (!ts.isExpressionStatement(statement) || !ts.isCallExpression(statement.expression)) continue;
    const call = statement.expression;
    if (!ts.isIdentifier(call.expression) || !["test", "fixtureBundleNativeTest"].includes(call.expression.text)) continue;
    assert.equal(call.arguments.length, 2);
    assert.ok(ts.isStringLiteral(call.arguments[0]));
    assert.ok(ts.isArrowFunction(call.arguments[1]) || ts.isFunctionExpression(call.arguments[1]));
    cases.push({ name: call.arguments[0].text, callback_sha256: astDigest([call.arguments[1]], source) });
  }
  const profile = descendantNodes(registration, (node) => ts.isVariableDeclaration(node)
    && ts.isIdentifier(node.name) && node.name.text === "REFERENCE_SDK_BUNDLE_PROFILES");
  const nested = profile.flatMap((node) => descendantNodes(node, (child) => ts.isPropertyAssignment(child)
    && child.name.getText(source) === "name" && ts.isStringLiteral(child.initializer))
    .map((child) => child.initializer.text));
  const calls = assertions(registration);
  return {
    imports_sha256: astDigest(imports, source), prefix_sha256: astDigest(prefix, source),
    body_sha256: astDigest(body, source), assertion_count: calls.length,
    assertions_sha256: astDigest(calls, source), cases, nested_case_names: nested,
  };
}
export function inspectEntrypoint(sourceText) {
  const source = parseSuiteSource(sourceText);
  return astDigest([...source.statements], source);
}
