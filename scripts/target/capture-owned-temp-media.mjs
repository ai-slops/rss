import { readFile, access, cp, mkdir, writeFile } from "node:fs/promises"
import { setTimeout as delay } from "node:timers/promises"

const [logPath, directory, policy] = process.argv.slice(2)
if (!logPath || !directory || !["during-owned-test", "after-owned-test"].includes(policy)) throw new Error("Expected owned trace, new private capture directory and snapshot policy")
await mkdir(directory, { mode: 0o700 })
let roots = []
let eventGateReached = false
const deadline = Date.now() + 5000
do {
  const log = await readFile(logPath, "utf8")
  roots = [...new Set([...log.matchAll(/(\/tmp\/\.tmp[A-Za-z0-9]+)\/[0-9a-f-]+\/segment-\d+\.mkv/g)].map(value => value[1]))]
  eventGateReached = log.includes("Caps changes are not supported by Matroska") && log.includes("splitmuxsink-fragment-closed")
  if (roots.length > 0 && eventGateReached || policy === "after-owned-test") break
  await delay(50)
} while (Date.now() < deadline)
const results = []
for (const [index, source] of roots.entries()) {
  try {
    await access(source)
    const destination = `${directory}/media-${index}`
    await cp(source, destination, { recursive: true, errorOnExist: true, force: false })
    results.push({ source, destination, captured: true })
  } catch (error) {
    if (error.code !== "ENOENT") throw error
    results.push({ source, captured: false, reason: "notPresentAtSnapshot" })
  }
}
await writeFile(`${directory}/capture.json`, JSON.stringify({ policy, mayContainOpenOrInterruptedFiles: true,
  eventGateReached, notDecodePromotion: true, results }, null, 2), { mode: 0o600 })
console.log(JSON.stringify({ policy, roots: roots.length, captured: results.filter(value => value.captured).length }))
