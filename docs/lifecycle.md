# Native preview and smoke ownership

The project builds `live-production-supervisor`, a small Linux-only CLI. It:

- starts each command in a private process group;
- becomes a child subreaper so detached/double-fork descendants remain owned and can be reaped;
- preserves the command exit code, returns 124 on deadline, and handles INT, TERM and HUP;
- drains its owned tree on normal completion, failure, deadline and signals;
- escalates from TERM to KILL after a bounded grace interval and waits for adopted children;
- watches its own launcher generation, not just its PID, and stops when that launcher disappears.

The native application additionally watches its owning supervisor's PID and process generation. If that owner is abruptly killed, it enters the same graceful coordinator/output shutdown path as SIGTERM. Direct launches watch their launcher; already orphaned direct launches are rejected. System service deployments use the supervisor in `deploy/live-production.service` rather than relying on an untracked background shell.

`scripts/serve.sh`, `scripts/dev.sh`, shell media/output gates, the acceptance entrypoint, and browser runners use this ownership path. Smoke scripts build first and execute the native binary directly instead of treating Cargo as the application process. Browser runners close Playwright in `finally`; the supervisor also owns escaped browser descendants. There is no `nohup`/`setsid` application-launch path in project runners.

Every `MediaEngine` now retains and joins its owner thread, for synthetic and SRT sources alike. This fixes a reproduced post-assertion native SRT teardown crash caused by process exit racing the media owner. Source handlers/probes are detached before the core pipeline enters NULL. The core graph does not wait for EOS; independent output owners are responsible for draining their muxers before coordinator shutdown.

## Regression

```sh
LIVE_PRODUCTION_LIFECYCLE_EXPECT_ZERO=1 ./scripts/lifecycle-acceptance.sh
```

Run the zero-baseline form with preview stopped. Normal acceptance can also verify that an existing, separately supervised preview remains unchanged.

The regression inspects actual `/proc` executable identities and process generations, including deleted executables.

For explicitly authorized emergency cleanup of **all project native applications**, including a supervised preview:

```sh
node scripts/cleanup-runtimes.mjs --all
```

This is not part of normal acceptance and must not be used to stop an unrelated or user-owned production session implicitly. Normal runner cleanup targets only descendants it owns. Uninterruptible kernel I/O and host shutdown remain operating-system boundaries; cleanup failures are errors, not successful leak checks.
