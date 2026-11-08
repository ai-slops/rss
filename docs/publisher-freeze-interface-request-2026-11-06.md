# Minimal genuine publisher-only fault interface requested from primary

## Why no verifier-side substitute is accepted

Inspection of `publisher.hpp`, `publisher_runtime.hpp/.cpp`,
`module_runtime_actions.cpp` and `recovery_fixture.cpp` confirms:

- The private fixture's `freezePublisher` branch only returns
  `externalProcessRequired`.
- `Publisher` exposes initialize/start/stop; `PublisherRuntime` exposes
  configure/start/stop/restart and a forced-failure counter. Its live Publisher
  and that Publisher's output/encoder/view handles are private.
- Pause of the recovery action dispatcher pauses command execution, not media.
- Whole-process SIGSTOP suspends the HTTP controller too; it is an offline
  boundary. Bridge DATA pause is a receiver/network fault, not an independent
  publisher freeze.

There is no existing test-only safe API that can hold only one real Publisher's
encoded A/V delivery while controller polling and OBS remain live. The verifier
will not use ptrace/thread suspension, global graphics/audio stops, private
memory mutation, or a callback-table patch to fake that capability. Implementing
the per-instance seam requires the owner of the Publisher lifecycle. No normal
production code is changed by this request.

## Smallest requested source interface

Private C++ test entrypoint, reachable only through the existing private
recovery fixture, with no controller HTTP/operator/public API extension:

```cpp
struct PublisherFaultState {
  uint64_t publisher_instance;
  bool delivery_held;
  uint64_t video_packets_delivered;
  uint64_t audio_packets_delivered;
  int64_t last_video_pts;
  int64_t last_audio_pts;
};

bool ModuleRuntime::fault_hold_publisher_delivery(
    uint32_t duration_ms, std::string &error);
PublisherFaultState ModuleRuntime::fixture_publisher_fault_state() const;
```

The ModuleRuntime method delegates to its current PublisherRuntime/Publisher.
It must not restart, stop, reconfigure, mute, or disable a source as the fault
operation. Primary chooses the implementation at the private output's encoded
delivery boundary; this request does not assume an unsupported libobs pause API.

Required admission/lifetime constraints:

1. Enabled only by a successfully parsed mode-0600 local fixture, for its
   configured loopback source/publisher; ordinary plugin use cannot invoke it.
2. Require the current Publisher actually sending. Reject absent/stopped or
   partially initialized instances. Duration is bounded, for example 100–5000 ms;
   the native case uses 4000 ms to cross the 3-second stall debounce.
3. Hold both video and audio delivery on only that Publisher instance. Other
   views/outputs, OBS main stream/record state, sources, global audio/rendering,
   Qt event processing and controller HTTP polling remain unchanged.
4. Do not hold the state/lifecycle mutex while waiting. Stop/restart/unload must
   cancel/wake the gate. A generation-fenced automatic expiry must never touch a
   replacement/released Publisher or reverse manual Stop.
5. Report the fault applied only after the actual gate is active, not merely
   because a command requested it. No packet bytes, credentials or URL in telemetry.

## Private fixture/report wiring

Replace the no-op `freezePublisher` behavior with the method above. Acknowledge
`executedCommandResult:"publisherDeliveryHeld"` only after admission/application.
Add the nonsecret `PublisherFaultState` and applied publisher-instance identity
to the fixture report. Unsupported or rejected requests remain non-pass outcomes.

The existing fixture state/dispatcher tests should cover init rejection,
concurrent Stop/expiry, released/recreated instances, and unload cancellation.
This is not a new companion service or a generic remote execution facility.

## Same-verifier acceptance after an exact primary checkpoint

1. Require genuine OBS and real authenticated receiver video, audio and PTS
   progress, including strict decoded fixture content/RMS. Existing silent OBS
   audio is still a separate failed prerequisite, not waived by this interface.
2. Apply the fault to the observed current publisher instance. Sample two strict
   intervals proving its delivered video/audio counters/PTS stop while controller
   poll sequence advances and it remains online. Check the real receiver stalls.
3. Let the existing authorized bounded recovery policy recreate the Publisher.
   Require a newer publisher instance and receiver continuity generation, two
   strict advancing A/V/PTS observations, and strict recorded expected content.
4. Prove duplicate delivery executes once and Stop cancels a held/queued action.
   An expired old-instance callback cannot resume media after Stop or affect the
   replacement. Main/public-output state remains unchanged.
5. Retain packet/counter timings, source/run/process/intent epochs, private report,
   media and strict decoder outcomes. No bridge DATA pause is used for this case.

Until this seam and its exact checkpoint exist, `publisherTransportFreeze` stays
required/unavailable and the full recovery result cannot be promoted to pass.
The existing receiver bridge mechanism remains useful, explicitly separate and
not counted twice as independent physical fault coverage.
