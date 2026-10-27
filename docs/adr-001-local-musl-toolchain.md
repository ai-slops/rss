# ADR 001: Native-musl project-local development dependencies

Status: accepted, 2026-10-27.

The product targets headless Ubuntu, but the assigned ordinary workspace container is Alpine/musl and initially exposes no Rust, C compiler, GStreamer or FFmpeg. A downloaded glibc micromamba binary did not load. The dependency worker proved that native Alpine packages installed in a non-root, project-local APK prefix compile C and Rust bindings and execute GStreamer and FFmpeg.

Use the pinned Alpine 3.23 package set under `.tools/sysroot` for this workspace. `scripts/bootstrap.sh` installs without package scripts or host reconfiguration; `scripts/env.sh` supplies local compiler wrappers, library/plugin paths and Cargo cache. Rust, React, SQLite and the official GStreamer bindings remain the product architecture. This does not establish Ubuntu deployment or hardware encoder performance.

On an Ubuntu deployment, install the documented native Ubuntu dependencies through the deployment administrator's normal process, use the locked Cargo/npm dependencies, and rerun the software acceptance profile. Do not use Alpine shared libraries as an Ubuntu deployment bundle. Record target-machine versions and performance separately.
