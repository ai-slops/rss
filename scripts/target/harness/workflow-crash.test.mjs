import assert from "node:assert/strict"
import test from "node:test"
import { mkdtemp, open, rm, stat } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { stopOnFragmentCreation } from "./workflow-crash.mjs"

test("stops synchronously when the target recording fragment is created", async (context) => {
  // Given
  const directory = await mkdtemp(join(tmpdir(), "live-production-crash-sync-"))
  context.after(() => rm(directory, { recursive: true, force: true }))
  const fragmentName = "segment-00003.mkv"
  let stoppedPath = null
  const pending = stopOnFragmentCreation({
    directory,
    fragmentName,
    timeoutMs: 1_000,
    stop: (path) => { stoppedPath = path },
  })

  // When
  const fragment = await open(join(directory, fragmentName), "wx")
  await pending

  // Then
  assert.equal(stoppedPath, join(directory, fragmentName))
  assert.equal((await stat(stoppedPath)).size, 0)
  await fragment.close()
})
