import { chmod, lstat, mkdir, readFile, readdir, rename, writeFile } from "node:fs/promises"
import { dirname, resolve } from "node:path"
import { randomUUID } from "node:crypto"

export async function ensurePrivateDirectory(path) {
  await mkdir(path, { recursive: true, mode: 0o700 })
  await chmod(path, 0o700)
  return resolve(path)
}

export async function validateFreshEvidenceDirectory(path, expectedPrivateEntries) {
  const absolute = resolve(path)
  const rootStat = await lstat(absolute)
  if (!rootStat.isDirectory() || rootStat.isSymbolicLink()) throw new Error("Evidence root must be a real private directory")
  const rootEntries = (await readdir(absolute)).sort()
  if (rootEntries.length !== 1 || rootEntries[0] !== "private") {
    throw new Error("Evidence root is not fresh; only the private directory is allowed")
  }
  const privateDirectory = resolve(absolute, "private")
  const privateStat = await lstat(privateDirectory)
  if (!privateStat.isDirectory() || privateStat.isSymbolicLink()) throw new Error("Evidence private path must be a real directory")
  const privateEntries = (await readdir(privateDirectory)).sort()
  if (JSON.stringify(privateEntries) !== JSON.stringify([...expectedPrivateEntries].sort())) {
    throw new Error(`Evidence private directory is stale: ${privateEntries.join(",") || "empty"}`)
  }
  await chmod(absolute, 0o700)
  await chmod(privateDirectory, 0o700)
  return { evidenceDirectory: absolute, privateDirectory }
}

export async function writePrivateJson(path, value) {
  await ensurePrivateDirectory(dirname(path))
  const temporary = `${path}.${randomUUID()}.tmp`
  await writeFile(temporary, `${JSON.stringify(value, null, 2)}\n`, { mode: 0o600 })
  await rename(temporary, path)
  await chmod(path, 0o600)
  return resolve(path)
}

export async function readJson(path) {
  return JSON.parse(await readFile(path, "utf8"))
}

export function publicPath(path, evidenceDirectory) {
  const absolute = resolve(path)
  const root = `${resolve(evidenceDirectory)}/`
  return absolute.startsWith(root) ? absolute.slice(root.length) : "<private-path>"
}

export function redactEndpoint(endpoint) {
  return {
    sourceId: endpoint.sourceId,
    protocol: endpoint.protocol,
    bindAddress: endpoint.bindAddress,
    port: endpoint.port,
    bufferMs: endpoint.bufferMs,
    pbkeylen: endpoint.pbkeylen,
    streamIdPresent: typeof endpoint.streamId === "string" && endpoint.streamId.length > 0,
    passphrasePresent: typeof endpoint.passphrase === "string" && endpoint.passphrase.length > 0,
  }
}
