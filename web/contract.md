# Web Coordinator API Contract

The browser uses same-origin cookie authentication and sends `X-Live-Production: 1` on every mutation. Credentials and platform secrets never enter URLs or browser persistence.

## Authentication

- `POST /api/v1/auth/login` with `{ "password": string }` sets the `HttpOnly`, `SameSite=Strict` session cookie.
- `POST /api/v1/auth/logout` invalidates the current session.
- `401` means the UI returns to sign-in. `403` means the session is authenticated but lacks authority.

## Authoritative State

- `GET /api/v1/snapshot` returns the Zod-validated shape in `src/api/schema.ts`.
- `GET /api/v1/events?after=<cursor>` is an authenticated SSE stream.
- Each ordinary event advances the cursor and causes the first-release UI to refresh the snapshot.
- A `gap` event means the cursor is no longer continuous and requires a fresh snapshot before controls are enabled.
- Control transport health, preview media health, source media activity, Program render health, and output media progress are independent observations.

## Commands

`POST /api/v1/commands` receives:

```json
{
  "id": "opaque idempotency key",
  "kind": "program.take",
  "targetId": "scene id",
  "expectedRevision": "exact target revision",
  "expectedEngineRunId": "current engine run",
  "payload": {}
}
```

It returns `accepted`, `applied`, `rejected`, or `failed` with the command ID, accepted time, optional applied time, actual revision, and reason. Command IDs are deduplicated for the coordinator's retention window. The browser queries state after uncertain outcomes and never blindly replays a Take, mute, output start, or output stop.

Required command kinds used by the UI:

- `scene.create`, `scene.rename`, `scene.duplicate`, `scene.move`, `scene.delete`
- `scene.save`, `scene.cue`, `scene.take`
- `program.cuePrevious`
- `source.create`, `source.configure`, `source.enable`, `source.delete`
- `audio.apply`, `audio.set`
- `timing.apply`
- `output.create`, `output.start`, `output.stop`
- `plugin.command`

`scene.save` carries `{ scene }` and uses `expectedRevision` for optimistic concurrency. An exact `scene.take` carries `payload.sceneRevision` and `payload.expectedProgramRevision`, sets `expectedRevision` to the target scene revision, and includes `expectedEngineRunId`. `output.start` carries explicit `confirmPublic`; `output.stop` does not. The coordinator must leave the previous Program intact when preparation or application fails.

Expected rejection codes include `revision_conflict`, `engine_run_changed`, `dependency_blocked`, `infeasible_timing`, `output_reconfiguration_required`, `unauthorized`, and `unavailable`. Responses include a useful operator-facing message and the current authoritative revision when applicable.

## Browser Media

The snapshot declares Preview media state and saved/rendered/visible revisions independently. The media transport may use authenticated WebRTC signaling at `/api/v1/preview/sessions`; the UI must not claim a displayed revision unless the backend and browser supply reliable frame correlation. Browser monitoring begins muted and Listen state remains browser-local.
