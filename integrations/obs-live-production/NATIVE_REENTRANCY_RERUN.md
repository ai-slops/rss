# OBS fixture reentrancy rerun

Use the existing Ubuntu verifier and retained native environment after the
ordinary crash-fix checkpoint is published. Do not create another VM or reuse
an evidence directory from the crashing run.

## Build and focused tests

```sh
cmake -S integrations/obs-live-production \
  -B var/obs-plugin-build \
  -DCMAKE_BUILD_TYPE=RelWithDebInfo
cmake --build var/obs-plugin-build --parallel 2
ctest --test-dir var/obs-plugin-build --output-on-failure
cmake --install var/obs-plugin-build --prefix var/obs-plugin-stage
```

The CTest run must execute both focused tests:

- `obs-live-production-fixture-state-test` covers synchronous callback dispatch
  during negative construction, initialization failure classification, stale
  task generations, nested transitions and cleanup invalidation.
- `obs-live-production-fixture-dispatcher-test` uses the same Qt dispatcher as
  the controller. It pauses enqueue while stop races, proves the queued fixture
  callback is removed with its receiver, and proves an unrelated Qt meta-call
  is not globally drained.

## Exact native harness rerun

Use the same native release daemon, supervisor, OBS 30.0.2.1, software GL and
Xvfb path as the retained verifier:

```sh
scripts/target/rehearsal.sh \
  --plugin-dir "$PWD/var/obs-plugin-stage" \
  --evidence-dir "$PWD/var/obs-reentrancy-rerun" \
  --server-binary "$PWD/target/release/live-production" \
  --supervisor-binary "$PWD/target/release/live-production-supervisor" \
  --obs-binary /usr/bin/obs \
  --deadline-seconds 60
```

## Required evidence

Return the complete private rehearsal report and nonsecret process logs. The
run must show all of the following rather than only an OBS exit code:

1. OBS does not exit with signal 11 or exit 139 while negative publishers are
   initialized.
2. The fixture report lists both primary senders as `sending`.
3. `badAuth` and `duplicateSender` are `rejected` only from actual output start,
   stop-error or reconnect signals. An initialization failure must remain
   `fixtureError`, never `rejected`.
4. The report contains the declared color/frame-counter/tone metadata and no
   credentials.
5. OBS shutdown leaves no queued plugin evaluation callback, crash, core file
   or plugin-owned process.
6. CTest reports both fixture state and fixture dispatcher tests passing under
   the native Qt 6.4.2 build used by OBS.

If the harness fails later than the fixture-report gate, retain the report and
OBS log and identify the next failing gate separately. Do not collapse a later
receiver, recording or recovery failure into this crash checkpoint.
