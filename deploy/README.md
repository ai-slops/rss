# Container deployment with router port forwarding

Build on native Ubuntu 24.04 after `scripts/ubuntu-dependencies.sh`:

```sh
bash scripts/package-server.sh "$PWD/var/container"
docker compose --env-file var/production/deploy.env -f deploy/compose.yml build
docker compose --env-file var/production/deploy.env -f deploy/compose.yml up -d
```

The packaging output must be a new directory. The container includes the pinned
rswebrtc plugin, runs the supervisor as an unprivileged user, and stores project
configuration and credentials under `var/production/state`. Back up that directory
privately. Restarting invalidates browser sessions and stops output sessions.

Before starting, create `var/production` and `var/production/state` with mode 0700,
owned by the UID/GID used below. Create a mode-0600 `deploy.env` there:

```dotenv
LIVE_LAN_IP=10.10.142.3
LIVE_UID=1000
LIVE_GID=1000
LIVE_HTTPS_NAMES=https://YOUR_PUBLIC_IP:8443
LIVE_TLS_DEFAULT_NAME=YOUR_PUBLIC_IP
```

The default proxy image is `caddy:2`; an existing compatible image can be selected
with `LIVE_CADDY_IMAGE`. Create a mode-0600 `preview-network.json` in the same
directory, readable by the configured UID:

```json
{"stunServer":"stun://stun.l.google.com:19302","nativeUdpRange":{"min":50000,"max":50100},"iceTransportPolicy":"all"}
```

Forward these ports on the router to `LIVE_LAN_IP`, preserving their numbers:

| Protocol | External and internal ports | Purpose |
| --- | --- | --- |
| TCP | 8443 | HTTPS UI, API and preview signalling |
| UDP | 9000–9015 | Up to 16 default SRT input ports |
| UDP | 50000–50100 | WebRTC preview media |

Open `https://YOUR_PUBLIC_IP:8443`. A sender does not need Tailscale. The router's
WAN IP must actually be publicly reachable; an upstream NAT also needs forwarding.
Test from a separate internet connection, since router hairpin NAT support varies.
Sources must bind to `0.0.0.0` and use a published SRT port. Additional sources or
custom ports require matching Docker and router mappings.

The IP-address deployment uses a private Caddy CA. Export only its public root:

```sh
docker compose --env-file var/production/deploy.env -f deploy/compose.yml \
  cp https:/data/caddy/pki/authorities/local/root.crt var/production/https-root.crt
openssl x509 -in var/production/https-root.crt -noout -fingerprint -sha256
```

Verify the fingerprint through the trusted operator channel before trusting that
root on a client. Browser previews and recovery HTTPS clients need a trusted TLS
connection. [Caddy documents private CA trust here](https://caddyserver.com/docs/automatic-https#local-https).
Keep the CA private key in `caddy-data` private. With a public domain, replace
`tls internal` with the appropriate public certificate configuration.

The generated sign-in password is `var/production/state/operator-password`.
Read it privately. Never include it in a publisher bundle. Use the authenticated
source publishing endpoint to generate a scoped bundle as described in the
[OBS integration](../integrations/obs-live-production/README.md).

STUN and forwarded UDP enable direct preview connectivity where the client network
allows it. Restricted or symmetric NAT clients may additionally need TURN, configured
through the [preview network settings](../docs/remote-monitoring-rehearsal.md).
HTTPS success alone does not verify SRT or WebRTC media delivery.

Inspect health and bounded service logs with `docker compose ... ps` and
`docker compose ... logs --tail=100`. Stop only this deployment with
`docker compose --env-file var/production/deploy.env -f deploy/compose.yml down`.
The private state and CA directories remain on disk.
