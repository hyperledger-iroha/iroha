// Fixed installed qualification bootstrap. Parent still owns original inputs,
// process/pipes, runtime/native manifest verification and publication authority.
// No SDK/native qualification is implied by merely importing/parsing this code.
import { run } from "node:test";
import { prepareChild } from "./sorafs_javascript_child_session.mjs";

if (process.argv.length !== 3) throw new Error("fixed child requires exactly the original input SHA256");
const owner = prepareChild(process.argv[2]);
let primary, raw;
try {
  const stream = run({ files: [owner.entryPath], isolation: "none", concurrency: false });
  for await (const event of stream) owner.accept(event);
  const observation = owner.finishAfterEof();
  raw = Buffer.from(JSON.stringify(observation) + "\n");
  if (raw.length > 8 * 1024 * 1024) throw new Error("child observation exceeds its fixed report ceiling");
  // TODO: the source-owned parent must retain every actual pipe byte, require
  // exact frame-at-EOF/process success and rederive original inputs. Until that
  // join exists this stream is component observation, never release evidence.

} catch (error) { primary = error; }
try { owner.close(); } catch (cleanup) {
  if (primary) throw new AggregateError([primary, cleanup], "child and cleanup failed");
  throw cleanup;
}
if (primary) throw primary;
process.stdout.write("SORAFS_JAVASCRIPT_CHILD_V1 " + raw.toString("base64") + "\n");
