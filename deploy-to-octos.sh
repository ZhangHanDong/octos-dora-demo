#!/usr/bin/env bash
# Build the feetech-omni-car bridge and install the skill into an octos plugin
# dir so an octos gateway/chat picks up its tools (init_motors / move_base /
# stop / robot_estop) via the binary protocol.
#
# Usage:
#   bash deploy-to-octos.sh [OCTOS_HOME]
#     OCTOS_HOME  octos data dir whose `skills/` is scanned (default: ~/.octos)
#                 For the Matrix/robrix gateway, pass the same --data-dir you run
#                 `octos serve` with.
#
# After it runs: (re)start your octos gateway/serve so it registers the tools,
# and make sure OMNI_CAR_PORT (default /dev/ttyACM0) + serial perms are set in
# the gateway's environment.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OCTOS_HOME="${1:-$HOME/.octos}"
SKILL_NAME="feetech-omni-car"
DST="$OCTOS_HOME/skills/$SKILL_NAME"

echo "[deploy] building omni-car-bridge (release)…"
( cd "$HERE/bridge" && cargo build --release )
BIN="$HERE/bridge/target/release/omni-car-bridge"
[ -x "$BIN" ] || { echo "[deploy] ERROR: build produced no binary at $BIN" >&2; exit 1; }

echo "[deploy] installing skill -> $DST"
mkdir -p "$DST"
cp "$HERE/manifest.json" "$HERE/SKILL.md" "$DST/"
# octos resolves the skill executable by name candidates [manifest.name,
# dir_name, "main"] (it does NOT read manifest.json "binary"). Install the
# binary as "main" so the loader matches deterministically — not via the
# "only executable in the dir" read_dir fallback.
cp "$BIN" "$DST/main"
chmod +x "$DST/main"

echo "[deploy] done. installed files:"
ls -la "$DST"

cat <<NOTE

[deploy] next steps
-------------------
1. Serial: plug the car's USB-TTL; default port is /dev/ttyACM0 (override with
   OMNI_CAR_PORT). Give your user serial access:  sudo usermod -aG dialout \$USER
   (re-login), or run octos with access to the device.
2. Smoke-test the binary directly (no octos), with the car powered:
     echo '{}' | OMNI_CAR_PORT=/dev/ttyACM0 "$DST/main" init_motors
     echo '{"vx":500,"vy":0,"omega":0,"duration_secs":2}' | OMNI_CAR_PORT=/dev/ttyACM0 "$DST/main" move_base
     echo '{}' | "$DST/main" robot_estop
3. Restart your octos gateway so it registers the tools, e.g.:
     OMNI_CAR_PORT=/dev/ttyACM0 octos serve --data-dir "$OCTOS_HOME"   # Matrix/robrix
   or for a quick local check:
     OMNI_CAR_PORT=/dev/ttyACM0 octos chat --data-dir "$OCTOS_HOME" -m "让小车前进2秒"
4. From robrix: message the bot "让小车原地旋转3秒后停止" — it sequences
   init_motors -> move_base -> stop.
NOTE
