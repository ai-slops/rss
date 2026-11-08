import { poll } from "./processes.mjs"

export async function createSrtSources(api, ports) {
  const definitions = [
    { key: "red", name: "OBS Red 440", color: "red", toneHz: 440, audioMixIndex: 0, port: ports[0] },
    { key: "blue", name: "OBS Blue 880", color: "blue", toneHz: 880, audioMixIndex: 1, port: ports[1] },
  ]
  for (const definition of definitions) {
    const snapshot = await api.snapshot()
    await api.command("source.create", snapshot.project.id, {
      name: definition.name,
      adapter: { kind: "srt", bindAddress: "127.0.0.1", port: definition.port, bufferMs: 120 },
    }, { snapshot })
  }
  const snapshot = await api.snapshot()
  return definitions.map((definition) => {
    const source = snapshot.project.sources.find((candidate) => candidate.name === definition.name)
    if (source === undefined) throw new Error(`Created source ${definition.name} missing from snapshot`)
    return { ...definition, source }
  })
}

export async function waitForConnectedSources(api, sources, timeoutMs) {
  let observed
  await poll(async () => {
    observed = await api.snapshot()
    return sources.every(({ source }) => {
      const current = observed.project.sources.find((candidate) => candidate.id === source.id)
      return current?.tracks.every((track) => track.connection === "connected") === true
    })
  }, timeoutMs, "both OBS source tracks to connect")
  return observed
}

export async function configureProduction(api, sources) {
  const before = await api.snapshot()
  const programRevisionBeforeDraft = before.program.programRevision
  for (const { source } of sources) {
    const snapshot = await api.snapshot()
    const scene = previewScene(snapshot)
    await api.command("scene.item.add", scene.id, { sourceId: source.id }, {
      snapshot,
      expectedRevision: scene.revision,
    })
  }
  let snapshot = await api.snapshot()
  const scene = previewScene(snapshot)
  const width = snapshot.project.canvas.width
  const height = snapshot.project.canvas.height
  const items = scene.items.map((item, index) => ({
    ...item,
    rect: { x: index * width / 2, y: 0, width: width / 2, height },
  }))
  await api.command("scene.save", scene.id, { scene: { ...scene, items } }, {
    snapshot,
    expectedRevision: scene.revision,
  })
  snapshot = await api.snapshot()
  if (snapshot.program.programRevision !== programRevisionBeforeDraft) {
    throw new Error("Draft edits changed Program before Take")
  }
  const routes = sources.map(({ source }) => ({
    trackId: trackId(source, "audio"), gainDb: 0, muted: false, routed: true,
  }))
  await api.command("audio.apply", snapshot.project.id, { routes }, {
    snapshot,
    expectedRevision: snapshot.project.revision,
  })
  snapshot = await api.snapshot()
  const corrections = sources.flatMap(({ key, source }) => source.tracks.map((track) => ({
    trackId: track.id,
    correctionMs: key === "red" ? 250 : 0,
  })))
  await api.command("timing.apply", snapshot.project.id, { holdbackMs: 500, corrections }, {
    snapshot,
    expectedRevision: snapshot.project.revision,
  })
  snapshot = await api.snapshot()
  const liveScene = previewScene(snapshot)
  await api.command("program.take", liveScene.id, {
    sceneId: liveScene.id,
    revision: liveScene.revision,
  }, { snapshot, expectedRevision: snapshot.program.programRevision })
  const after = await api.snapshot()
  if (after.project.audioRoutes.length !== 2 || after.program.sceneId !== liveScene.id) {
    throw new Error("Take did not preserve explicit shared audio routes")
  }
  return { snapshot: after, programRevisionBeforeDraft }
}

export async function createRecordingOutputs(api, sources) {
  const specifications = [
    { name: "Target Program", adapter: { kind: "programRecording", segmentSeconds: 1 } },
    ...sources.map(({ key, source }) => ({
      name: `Target clean ${key}`,
      adapter: { kind: "sourceRecording", sourceId: source.id, segmentSeconds: 1 },
    })),
  ]
  for (const specification of specifications) {
    const snapshot = await api.snapshot()
    await api.command("output.create", snapshot.project.id, specification, { snapshot })
  }
  const snapshot = await api.snapshot()
  return specifications.map((specification) => {
    const output = snapshot.project.outputs.find((candidate) => candidate.name === specification.name)
    if (output === undefined) throw new Error(`Output ${specification.name} missing after creation`)
    return output
  })
}

export async function startOutputs(api, outputs) {
  for (const output of outputs) {
    const snapshot = await api.snapshot()
    await api.command("output.start", output.id, {}, { snapshot })
  }
}

export async function stopOutputs(api, outputs) {
  for (const output of outputs) {
    const snapshot = await api.snapshot()
    await api.command("output.stop", output.id, {}, { snapshot })
  }
  return api.snapshot()
}

export async function startEditorSession(api, name) {
  const snapshot = await api.snapshot()
  await api.command("plugin.command", "editor-session", { command: "session.start", text: name }, { snapshot })
}

export async function exportEditorSession(api) {
  const snapshot = await api.snapshot()
  const receipt = await api.command("plugin.command", "editor-session", { command: "session.export" }, { snapshot })
  return receipt.pluginResult
}

function trackId(source, kind) {
  const track = source.tracks.find((candidate) => candidate.kind === kind)
  if (track === undefined) throw new Error(`Source ${source.id} has no ${kind} track`)
  return track.id
}

function previewScene(snapshot) {
  const scene = snapshot.project.scenes.find((candidate) => candidate.id === snapshot.preview.sceneId)
  if (scene === undefined) throw new Error(`Preview scene ${snapshot.preview.sceneId} is absent from project scenes`)
  return scene
}
