#!/usr/bin/env bash
set -euo pipefail

GODOT_URL="https://downloads.godotengine.org/?version=4.7&flavor=dev2&slug=linux.x86_64.zip&platform=linux.64"
REPO_ROOT="$(cd "$(dirname "$0")" && pwd)"

docker run --rm --platform linux/amd64 \
  -v "$REPO_ROOT/test_plugin":/test_plugin:ro \
  -v "$REPO_ROOT/test_plugin/addons/sorkin/bin":/test_plugin/addons/sorkin/bin:ro \
  ubuntu:22.04 bash -c "
    set -euo pipefail
    apt-get update -q
    apt-get install -y --no-install-recommends \
      curl unzip libvpx7 libopus0 libglib2.0-0 \
      xvfb libasound2 libpulse0 ca-certificates 2>&1 | tail -3

    echo '--- Downloading Godot ---'
    curl -fsSL '${GODOT_URL}' -o /tmp/godot.zip
    unzip -q /tmp/godot.zip -d /tmp/godot
    GODOT=\$(find /tmp/godot -type f -name 'Godot*' | head -1)
    chmod +x \"\$GODOT\"

    echo '--- Running Godot headless ---'
    \"\$GODOT\" --path /test_plugin --editor --headless 2>&1 &
    PID=\$!
    sleep 10
    kill \$PID 2>/dev/null || true
  "
