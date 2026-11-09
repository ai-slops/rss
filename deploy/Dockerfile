# Build context: output of scripts/package-server.sh (contains app/).
FROM ubuntu:24.04 AS runtime
RUN apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
    ca-certificates curl ffmpeg libssl3t64 libglib2.0-0t64 libgstreamer1.0-0 \
    gstreamer1.0-tools gstreamer1.0-plugins-base gstreamer1.0-plugins-good \
    gstreamer1.0-plugins-bad gstreamer1.0-plugins-ugly gstreamer1.0-libav \
    gstreamer1.0-nice gstreamer1.0-x \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --uid 10001 --create-home studio \
    && install -d -o studio -g studio -m 700 /state
ENV LIVE_PRODUCTION_BIND=0.0.0.0:3000 \
    LIVE_PRODUCTION_STATE_DIR=/state \
    LIVE_PRODUCTION_WEB_DIST=/opt/live-production/web \
    LIVE_PRODUCTION_SECURE_COOKIE=true \
    GST_PLUGIN_PATH=/opt/live-production/lib \
    GST_REGISTRY=/tmp/gstreamer-registry.bin \
    RUST_LOG=info,gst_plugin_webrtc_signalling=warn
WORKDIR /opt/live-production
USER studio

FROM runtime AS server
COPY --chown=studio:studio app/ /opt/live-production/
RUN chmod -R a+rX bin lib web \
    && ldd bin/live-production | (! grep 'not found') \
    && ldd lib/libgstrswebrtc.so | (! grep 'not found') \
    && gst-inspect-1.0 webrtcsink >/dev/null
EXPOSE 3000/tcp 9000-9015/udp 50000-50100/udp
HEALTHCHECK --interval=15s --timeout=5s --start-period=30s --retries=3 \
    CMD curl --fail --silent http://127.0.0.1:3000/ >/dev/null || exit 1
ENTRYPOINT ["/opt/live-production/bin/live-production-supervisor", "--", "/opt/live-production/bin/live-production"]
