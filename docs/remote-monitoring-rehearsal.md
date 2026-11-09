# Remote Browser Monitoring Rehearsal

This rehearsal proves private browser monitoring separately from HTTPS control. A successful login
or working WebSocket proxy does not prove that WebRTC media can traverse the operator laptop,
public host, Ubuntu VM, and workspace network boundaries.

## Supported Native ICE Configuration

The server reads an optional private JSON file named by
`LIVE_PRODUCTION_PREVIEW_NETWORK_CONFIG`. The file must be a regular file with no group or other
permissions on Ubuntu, and is rejected if it exceeds 64 KiB. The legacy
`LIVE_PRODUCTION_STUN_SERVER` value remains supported when the file omits `stunServer`.

```json
{
  "stunServer": "stun://stun.example.net:3478",
  "turnServers": [
    {
      "scheme": "turns",
      "host": "turn.example.net",
      "port": 5349,
      "transport": "tcp",
      "username": "replace-with-private-username",
      "credential": "replace-with-private-credential"
    }
  ],
  "nativeUdpRange": { "min": 49160, "max": 49200 },
  "iceTransportPolicy": "all"
}
```

`turns` requires TCP. `iceTransportPolicy: "relay"` is rejected unless at least one TURN server is
configured. TURN credentials are percent-encoded in memory and are never included in pipeline
descriptions, capability responses, or evidence files.

The inspected development backend is rswebrtc `0.14.3-RELEASE` with GStreamer `1.26.9`.
`webrtcsink` exposes STUN, TURN arrays, relay policy, and the READY-state `consumer-added` signal.
The underlying ICE agent exposes `min-rtp-port` and `max-rtp-port`; the server applies the declared
range to every consumer before negotiation. Startup fails if the installed target backend cannot
enforce those properties. Report the actual plugin versions on the native Ubuntu host;
the development versions do not prove Ubuntu package availability.

## Authenticated Capability Check

After signing in, request:

```text
GET /api/v1/media/network-capabilities
```

The response reports the installed native plugin version, whether STUN/TURN/secure TURN are
configured, TURN transport kinds, the enforced UDP range, client limits, and the required host
connectivity class. It never returns STUN/TURN URLs, credentials, raw candidates, SDP, cookies, or
addresses. It always states that control HTTPS does not prove media and that a remote laptop has
not been verified until this rehearsal passes.

Current limits are six simultaneous browser media signaling connections and eight active private
source-audio monitor producers. Preview failure remains isolated from Program and outputs.

## Host Connectivity Choices

### Direct UDP Range

Use a fixed `nativeUdpRange` only after the host owner approves it. Preserve the same range through
every boundary:

1. Public host firewall/security policy allows the declared UDP range.
2. Public host NAT forwards that range to the Ubuntu VM address.
3. If the daemon is inside another workspace/container network, the VM forwards the same range to
   that runtime or uses an explicitly approved host-network arrangement.
4. The daemon process can bind the range; no other service shares it.
5. STUN or an equivalent host-owned mapping makes the reachable address visible in ICE.

Do not expose the embedded rswebrtc signaling ports. They stay on loopback behind the authenticated
HTTP/WebSocket proxy. `dim-expose` advertises HTTP(S) only and does not create the UDP forwarding
chain above. Never infer a UDP destination from the shared HTTPS proxy DNS name.

The range must cover concurrent Preview, Program, private Listen, retries, and ICE component
allocations. Six signaling clients is an application budget, not proof that a six-port range is
sufficient. Measure allocations on the Ubuntu target before narrowing host policy.

### TURN Relay

Use TURN when host UDP forwarding is unavailable, the public mapping is symmetric, or the laptop
network blocks direct ICE. The host owner supplies a TURN service reachable by the native server
and the laptop, with allocation and peer permissions suitable for this private application.
Outbound access to the TURN service must be allowed.

The current normal browser channel intentionally reports `browserIceServersConfigured: false`.
Native TURN can make the server side reachable, but a laptop that itself requires relay-only ICE
still needs a future authenticated client ICE configuration integration. Do not claim that case is
supported merely because native TURN is configured.

## Read-Only Laptop Check

Before the check, a local operator prepares an existing scene with active Preview and Program
video and at least one source audio track. The laptop check does not create/cue scenes, Take,
change routes, mute the shared mix, or start outputs.

1. Store the operator password in a laptop-local file readable only by its owner.
2. Install the repository's locked web dependencies and a normal Chrome browser.
3. Run the check from the rehearsal branch:

```sh
LIVE_PRODUCTION_URL=https://control.example.net/ \
LIVE_PRODUCTION_PASSWORD_FILE=/private/path/operator-password \
LIVE_PRODUCTION_MONITOR_REPORT=var/target-monitor-report.json \
node scripts/target/monitor-check.mjs
```

The default run is headed and uses no autoplay, ICE, local-network, fake-device, or candidate
bypass flags. It signs in with an in-memory cookie, verifies changing Preview and Program frames,
explicitly clicks private Listen and browser monitoring, and confirms inbound audio packets. It
then proves that Program revision and live audio routes did not change.

The report is mode `0600` and contains only peer indexes local to the report, receiver media kinds,
candidate types/protocol, connection states, decoded frame and packet counters, dimensions, mute
state, capability flags, and pass/fail outcomes. Private Listen passes only when the matched peer was
created after the click, receives only audio, and has inbound audio packets consistent with its ICE
entry. The report does not contain the password, cookies, SDP, peer IDs, candidate addresses/ports,
TURN endpoints, or project/source names.

## Result Interpretation

- **Control passes, media fails:** inspect ICE candidate types and host/TURN policy; do not change
  scene timing to compensate.
- **Video passes, private audio unavailable:** provision an existing source audio track locally,
  then rerun. Do not route that track merely to make Listen work.
- **TURN configured, no relay candidate:** verify TURN reachability and credentials privately;
  never paste them into evidence.
- **Direct range configured, ICE fails:** verify each public-host to VM to runtime UDP mapping and
  the target's actual bound sockets.

Real remote laptop behavior, Ubuntu hardware capacity, Wi-Fi/corporate firewall policy, and any
host forwarding or TURN service remain unverified until the host owner and laptop operator perform
this check on the actual route.
