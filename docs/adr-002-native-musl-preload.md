# ADR 002: isolate the native-musl ABI from a glibc compatibility preload

Status: accepted after measurement, 2026-10-27.

The worker supplies `/lib/libgcompat.so.0` through inherited `LD_PRELOAD`. The project-local compiler, GLib, GStreamer and FFmpeg are native-musl binaries. In the measured environment, loading that compatibility shim into native GLib 2.86.3 makes `g_strerror(EAGAIN)` return `NULL`, although libc `strerror(EAGAIN)` succeeds. libnice then cannot construct its ordinary would-block error and can abort at its send-result assertion.

The checked-in `scripts/probes/glib-errors.c` reproduced the failure without WebRTC, capture hardware, network credentials or browser flags. Running the same executable with only the compatibility preload removed returned `Resource temporarily unavailable`. Node 24 also ran normally in that child environment. Header and runtime GLib versions were identical.

`scripts/env.sh` now removes only the two observed spellings of that exact compatibility preload from the project development process environment. It does not alter the host, installed libraries, OpenCode, the DIM control plane, model configuration or router credentials, and it does not remove arbitrary preload libraries. Bootstrap executes the GLib contract probe so the incompatible ABI profile fails early rather than appearing to be an ICE or codec problem.

Ubuntu deployment uses native Ubuntu libraries and does not inherit this Alpine-specific workaround. Browser ICE and actual decoding still require their own fresh acceptance evidence; this ADR is not itself a media pass.
