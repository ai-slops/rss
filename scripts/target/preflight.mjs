import { resolve } from "node:path"
import { runPreflight } from "./harness/preflight.mjs"

const root = resolve(new URL("../..", import.meta.url).pathname)
const values = new Map()
for (let index = 2; index < process.argv.length; index += 2) values.set(process.argv[index], process.argv[index + 1])
const pluginDirectory = values.get("--plugin-dir")
const reportPath = values.get("--report")
if (pluginDirectory === undefined || reportPath === undefined) {
  process.stderr.write("Usage: scripts/target/preflight.sh --plugin-dir STAGE --report PRIVATE_JSON\n")
  process.exit(2)
}
const report = await runPreflight({
  pluginDirectory: resolve(pluginDirectory),
  reportPath: resolve(reportPath),
  serverBinary: resolve(values.get("--server-binary") ?? `${root}/target/release/live-production`),
  supervisorBinary: resolve(values.get("--supervisor-binary") ?? `${root}/target/release/live-production-supervisor`),
  obsBinary: values.get("--obs-binary") ?? "obs",
})
process.stdout.write(`${JSON.stringify({ status: report.status, report: resolve(reportPath) })}\n`)
process.exitCode = report.status === "pass" ? 0 : 1
