import assert from "node:assert/strict"
import test from "node:test"
import { mkdtemp, mkdir, writeFile } from "node:fs/promises"
import { join } from "node:path"
import { tmpdir } from "node:os"
import { validateFreshEvidenceDirectory } from "./private-files.mjs"

test("accepts only a fresh dry-run evidence root", async () => {
  const parent = await mkdtemp(join(tmpdir(), "lp-evidence-"))
  const root = join(parent, "run")
  await mkdir(join(root, "private"), { recursive: true, mode: 0o700 })
  const value = await validateFreshEvidenceDirectory(root, [])
  assert.equal(value.evidenceDirectory, root)
  await writeFile(join(root, "stale.json"), "{}")
  await assert.rejects(validateFreshEvidenceDirectory(root, []), /not fresh/u)
})

test("actual run permits only its new private preflight report", async () => {
  const parent = await mkdtemp(join(tmpdir(), "lp-preflight-"))
  const root = join(parent, "run")
  await mkdir(join(root, "private"), { recursive: true, mode: 0o700 })
  await writeFile(join(root, "private", "preflight.json"), "{}", { mode: 0o600 })
  await validateFreshEvidenceDirectory(root, ["preflight.json"])
  await writeFile(join(root, "private", "old-cookie.txt"), "stale", { mode: 0o600 })
  await assert.rejects(validateFreshEvidenceDirectory(root, ["preflight.json"]), /stale/u)
})
