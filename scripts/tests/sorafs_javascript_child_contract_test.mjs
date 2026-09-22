// Static source contracts: parse original assertion/connector ASTs, never load
// SDK implementations or a native addon. Run from the repository source tree.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { test } from "node:test";
import { inspectRegistration, parseSuiteSource, descendantNodes, printed } from "../../javascript/iroha_js/test/helpers/sorafsNativeSuiteContract.js";
const require = createRequire(new URL("../../javascript/iroha_js/package.json", import.meta.url));
const ts = require("typescript");
const read = (path) => readFileSync(new URL(path, import.meta.url), "utf8");
const contract = JSON.parse(read("../../javascript/iroha_js/test/fixtures/sorafs_native_suite_contract_v1.json"));
const session = read("../sorafs_javascript_child_session.mjs"), bootstrap = read("../sorafs_javascript_child.mjs");
const parse = parseSuiteSource;
function declaration(source, name) {
  const matches = descendantNodes(source, (n) => ts.isVariableDeclaration(n) && n.name.getText(source) === name);
  assert.equal(matches.length, 1, `sole ${name} declaration`);return matches[0].initializer;
}
function unwrap(node, source) {
  assert.ok(ts.isCallExpression(node));assert.equal(node.expression.getText(source),"Object.freeze");
  assert.equal(node.arguments.length,1);return node.arguments[0];
}
function strings(node) {assert.ok(ts.isArrayLiteralExpression(node));return node.elements.map((n)=>{assert.ok(ts.isStringLiteral(n));return n.text;});}
function fields(node, source) {
  assert.ok(ts.isObjectLiteralExpression(node));const result=new Map();
  for(const p of node.properties){assert.ok(ts.isPropertyAssignment(p)||ts.isShorthandPropertyAssignment(p));const name=p.name.getText(source);assert.ok(!result.has(name));result.set(name,ts.isPropertyAssignment(p)?p.initializer:p.name);}
  return result;
}
function picks(node, source) {
  if(ts.isCallExpression(node) && node.expression.getText(source)==="pick") {
    assert.equal(node.arguments.length,2);const owner=node.arguments[0].getText(source);
    assert.ok(["norito","sorafs","torii","api","hooks"].includes(owner));
    return strings(node.arguments[1]).map((name)=>[owner,name]);
  }
  const object=unwrap(node,source);assert.ok(ts.isObjectLiteralExpression(object));
  return object.properties.flatMap((p)=>{
    if(ts.isSpreadAssignment(p))return picks(p.expression,source);
    assert.ok(ts.isPropertyAssignment(p));assert.equal(p.name.getText(source),"getNativeBinding");
    assert.equal(p.initializer.getText(source),"this.#getter");return [["native","getNativeBinding"]];
  });
}
function checkContexts(text) {
  const source=parse(text), suites=unwrap(declaration(source,"SUITES"),source);
  assert.ok(ts.isArrayLiteralExpression(suites));
  assert.deepEqual(suites.elements.map(strings),contract.suites.map((row)=>[row.name,row.registration]));
  const contexts=declaration(source,"contexts");assert.ok(ts.isArrayLiteralExpression(contexts));assert.equal(contexts.elements.length,6);
  const expectedOwners=["norito","sorafs","sorafs",null,"native","sorafs"];
  for(const [i,row] of contract.suites.entries()){
    const entry=parse(read(`../../javascript/iroha_js/test/${row.name}.test.js`));
    const calls=descendantNodes(entry,(n)=>ts.isCallExpression(n)&&n.expression.getText(entry)===row.registration);
    assert.equal(calls.length,1);assert.equal(calls[0].arguments.length,1);
    const original=fields(calls[0].arguments[0],entry), actual=fields(contexts.elements[i],source);
    assert.deepEqual([...actual.keys()].sort(),[...original.keys()].sort());assert.equal(actual.get("test").getText(source),"test");
    const names=[...fields(original.get("subject"),entry).keys()], got=picks(actual.get("subject"),source);
    assert.deepEqual(got.map(([,name])=>name),names);
    for(const [owner,name] of got)assert.equal(owner,expectedOwners[i]??(name==="NetworkId"?"api":name==="TORII_TEST_NATIVE_BINDING"?"hooks":"torii"));
    if(original.has("fixtureRoot")){
      const expected=original.get("fixtureRoot");assert.ok(ts.isNewExpression(expected));assert.equal(expected.expression.getText(entry),"URL");
      const relative=expected.arguments[0];assert.ok(ts.isStringLiteral(relative));
      const call=actual.get("fixtureRoot");assert.ok(ts.isCallExpression(call));assert.equal(call.expression.getText(source),"fixture");
      assert.equal(call.arguments.length,1);assert.ok(ts.isStringLiteral(call.arguments[0]));
      assert.equal(`../../../fixtures/${call.arguments[0].text}/`,relative.text);
    }
    if(actual.has("nativeBinding")){assert.equal(actual.get("nativeBinding").getText(source),"this.#binding");assert.equal(actual.get("nativeBindingError").kind,ts.SyntaxKind.NullKeyword);}
    if(actual.has("temporaryRoot")){assert.equal(actual.get("temporaryRoot").getText(source),"this.#input.temporaryRoot");assert.equal(actual.get("repositoryRoot").getText(source),"directoryURL(this.#input.coreRoot)");}
  }
}
function checkBootstrap(text) {
  const source=parse(text), runs=descendantNodes(source,(n)=>ts.isCallExpression(n)&&n.expression.getText(source)==="run");
  assert.equal(runs.length,1);assert.equal(runs[0].arguments.length,1);
  const options=fields(runs[0].arguments[0],source);assert.deepEqual([...options.keys()],["files","isolation","concurrency"]);
  assert.equal(options.get("files").getText(source),"[owner.entryPath]");assert.equal(options.get("isolation").text,"none");assert.equal(options.get("concurrency").kind,ts.SyntaxKind.FalseKeyword);
  const loops=descendantNodes(source,ts.isForOfStatement);assert.equal(loops.length,1);assert.ok(loops[0].awaitModifier);
  assert.equal(loops[0].expression.getText(source),"stream");assert.equal(printed(loops[0].statement,source),"owner.accept(event);");
  const calls=(name)=>descendantNodes(source,(n)=>ts.isCallExpression(n)&&n.expression.getText(source)===name);
  for(const name of ["prepareChild","owner.finishAfterEof","owner.close","process.stdout.write"])assert.equal(calls(name).length,1);
  assert.ok(calls("prepareChild")[0].pos<runs[0].pos);
  assert.ok(loops[0].end<calls("owner.finishAfterEof")[0].pos);
  assert.ok(calls("owner.finishAfterEof")[0].end<calls("owner.close")[0].pos);
  assert.ok(calls("owner.close")[0].end<calls("process.stdout.write")[0].pos);
}
test("fixed child contexts project the six original source entrypoints exactly",()=>checkContexts(session));
test("closed actual TestsStream bootstrap consumes EOF before completion and emits only after cleanup",()=>checkBootstrap(bootstrap));
test("fixed entry has no injected subject or second owner reconstruction",()=>{
  const entry=parse(read("../sorafs_javascript_child_entry.mjs"));assert.equal(entry.statements.length,2);
  const [imp,call]=entry.statements;assert.ok(ts.isImportDeclaration(imp));assert.equal(imp.moduleSpecifier.text,"./sorafs_javascript_child_session.mjs");
  assert.equal(printed(call,entry),"await registerPreparedChild();");
  const source=parse(session), exports=source.statements.filter((n)=>n.modifiers?.some((m)=>m.kind===ts.SyntaxKind.ExportKeyword));
  assert.deepEqual(exports.map((n)=>[n.name.text,n.parameters.map((p)=>p.name.getText(source))]),[["prepareChild",["expectedInputSha256"]],["registerPreparedChild",[]]]);
});
test("all original 172 assertions and 55 case names remain in unchanged shared bodies",()=>{
  let assertions=0,cases=0;
  for(const {name,registration,statement_count,original_sha256,entrypoint_sha256,...expected} of contract.suites){
    const result=inspectRegistration(read(`../../javascript/iroha_js/test/sorafsNativeSuites/${name}.js`),registration,statement_count);
    assert.deepEqual(result,expected);assertions+=result.assertion_count;cases+=result.cases.length+result.nested_case_names.length;
  }
  assert.equal(assertions,172);assert.equal(cases,55);
});
for(const [name,from,to] of [
  ["wrong subject owner",'pick(norito, ["decodeCancelAssetLockV1"','pick(sorafs, ["decodeCancelAssetLockV1"'],
  ["missing subject",'"decodeCancelAssetLockV1", "encodeCancelAssetLockV1"','"decodeCancelAssetLockV1"'],
  ["fixture substitution",'fixture("sorafs_manifest")','fixture("sorafs_manifest/pdp")'],
  ["native substitution",'nativeBinding: this.#binding','nativeBinding: {}'],
  ["temporary fallback",'temporaryRoot: this.#input.temporaryRoot','temporaryRoot: "/tmp"'],
  ["suite reorder",'"cancelAssetLockV1", "registerCancelAssetLockV1Tests"','"other", "registerCancelAssetLockV1Tests"'],
])test(`context mutation rejected: ${name}`,()=>{const changed=session.replaceAll(from,to);assert.notEqual(changed,session);assert.throws(()=>checkContexts(changed));});
for(const [name,from,to] of [
  ["process isolation",'isolation: "none"','isolation: "process"'],
  ["concurrent execution",'concurrency: false','concurrency: true'],
  ["filtered cases",'concurrency: false','concurrency: false, testNamePatterns: ["one"]'],
  ["lost stream event",'owner.accept(event);','void event;'],
  ["invented stream owner",'files: [owner.entryPath]','files: ["other.mjs"]'],
])test(`bootstrap mutation rejected: ${name}`,()=>{const changed=bootstrap.replace(from,to);assert.notEqual(changed,bootstrap);assert.throws(()=>checkBootstrap(changed));});

// SameValue retains actual namespace references, accepts unchanged NaN and
// distinguishes signed zero; only its exact session predicate is accepted.
test("namespace retention uses SameValue on each original captured export",()=>{
  function check(text){
    const source=parse(text), rows=descendantNodes(source,(n)=>ts.isCallExpression(n)&&n.expression.getText(source)==="row.exports.every");
    assert.equal(rows.length,1);assert.equal(rows[0].arguments.length,1);
    const predicate=rows[0].arguments[0];assert.ok(ts.isArrowFunction(predicate));
    assert.equal(printed(predicate,source),"([key, value]) => Object.is(row.module[key], value)");
  }
  check(session);const changed=session.replace("Object.is(row.module[key], value)","row.module[key] === value");
  assert.notEqual(changed,session);assert.throws(()=>check(changed));
});
