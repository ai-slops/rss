import assert from "node:assert/strict"

export function assertProgramPreserved(before, after) {
  assert.deepEqual(content(after), content(before))
  if (after.engineRunId === before.engineRunId) assert.equal(after.program.programRevision, before.program.programRevision)
}

function content(snapshot) {
  assert.equal(typeof snapshot.engineRunId, "string")
  assert(snapshot.program.appliedScene !== undefined)
  return { projectId: snapshot.project.id, canvas: snapshot.project.canvas, assets: snapshot.project.assets,
    sceneId: snapshot.program.sceneId, sceneRevision: snapshot.program.sceneRevision, appliedScene: snapshot.program.appliedScene,
    audioRoutes: snapshot.project.audioRoutes.map(({ trackId, gainDb, muted, routed }) => ({ trackId, gainDb, muted, routed })) }
}
