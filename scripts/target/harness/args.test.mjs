import assert from "node:assert/strict"
import test from "node:test"
import { parseArgs } from "./args.mjs"

test("requires private plugin and evidence directories", () => {
  assert.throws(() => parseArgs([], "/repo"), /--plugin-dir is required/u)
  assert.throws(() => parseArgs(["--plugin-dir", "/plugin"], "/repo"), /--evidence-dir is required/u)
})

test("parses target rehearsal defaults without launching processes", () => {
  const options = parseArgs([
    "--plugin-dir", "/stage", "--evidence-dir", "/evidence", "--dry-run",
  ], "/repo")
  assert.equal(options.pluginDirectory, "/stage")
  assert.equal(options.evidenceDirectory, "/evidence")
  assert.equal(options.serverBinary, "/repo/target/release/live-production")
  assert.equal(options.webDist, "/repo/web/dist")
  assert.equal(options.dryRun, true)
})

test("preserves explicitly selected native binaries", () => {
  const options = parseArgs([
    "--plugin-dir", "/stage", "--evidence-dir", "/evidence",
    "--server-binary", "/native/server", "--supervisor-binary", "/native/supervisor",
    "--obs-binary", "/native/obs",
  ], "/repo")
  assert.equal(options.serverBinary, "/native/server")
  assert.equal(options.supervisorBinary, "/native/supervisor")
  assert.equal(options.obsBinary, "/native/obs")
})
