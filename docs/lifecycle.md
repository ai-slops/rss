# Native preview and smoke ownership

## Incident and repair

The operator measured 18 obsolete native application processes adopted by PID 1, each with 35–56 threads, saturating the eight-CPU workspace. Tracking a `cargo run` launcher PID and killing only that PID did not establish ownership of its native child. A further eight obsolete, deleted application executables were found and explicitly stopped during recovery.

The project now builds `live-production-supervisor`, a small Linux-only CLI. It:

- starts each command in a private process group;
- becomes a child subreaper so detached/double-fork descendants remain owned and can be reaped;
- preserves the command exit code, returns 124 on deadline, and handles INT, TERM and HUP;
- drains its owned tree on normal completion, failure, deadline and signals;
- escalates from TERM to KILL after a bounded grace interval and waits for adopted children;
- watches its own launcher generation, not just its PID, and stops when that launcher disappears.

The native application additionally watches its owning supervisor's PID and process generation. If that owner is abruptly killed, it enters the same graceful coordinator/output shutdown path as SIGTERM. Direct launches watch their launcher; already orphaned direct launches are rejected. System service deployments use the supervisor in `deploy/live-production.service` rather than relying on an untracked background shell.

`scripts/serve.sh`, `scripts/dev.sh`, shell media/output gates, the acceptance entrypoint, and browser runners use this ownership path. Smoke scripts build first and execute the native binary directly instead of treating Cargo as the application process. Browser runners close Playwright in `finally`; the supervisor also owns escaped browser descendants. There is no `nohup`/`setsid` application-launch path in project runners.

## Regression

```sh
LIVE_PRODUCTION_LIFECYCLE_EXPECT_ZERO=1 ./scripts/lifecycle-acceptance.sh
```

Run the zero-baseline form with preview stopped. Normal acceptance can also verify that an existing, separately supervised preview remains unchanged.

Observed after cleanup on 2026-10-27:

- Eleven actual native-app runs covered successful smoke completion, assertion failure, timeout, INT/TERM/HUP, repeated runs, unlinked/deleted executables and abrupt supervisor KILL.
- Native process count was **0 before and 0 after**. Each individual run restored baseline, not only the final run.
- Seven additional process-tree cases verified forced cleanup/reaping of an intentionally abandoned descendant, including a child that detached into another session.
- Five native binary unit tests passed, including owner-generation and exclusive-state ownership checks.
- Supervisor and production library/binary Clippy passed with warnings denied. Source-size and runner syntax checks passed.

Evidence comes from actual `/proc` executable identities and process generations, including the ` (deleted)` suffix; process-name matching alone is insufficient. No command-line arguments or credentials are printed by the process inspection helper.

For explicitly authorized emergency cleanup of **all project native applications**, including a supervised preview:

```sh
node scripts/cleanup-runtimes.mjs --all
```

This is not part of normal acceptance and must not be used to stop an unrelated or user-owned production session implicitly. Normal runner cleanup targets only descendants it owns. Uninterruptible kernel I/O and host shutdown remain operating-system boundaries; cleanup failures are errors, not successful leak checks.
