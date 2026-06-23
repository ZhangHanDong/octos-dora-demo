#!/usr/bin/env bash
# Build the feetech-omni-car bridge and install the skill so an octos
# gateway/serve (or chat) picks up its tools (init_motors / move_base / stop /
# robot_estop) via the binary protocol.
#
# IMPORTANT — octos 1.1.0 skill discovery (verified 2026-06-23):
#   The legacy global dir `<data-dir>/skills/` is NO LONGER scanned.
#   * gateway / serve  -> per-profile:  <data-dir>/profiles/<id>/data/skills/<skill>/
#   * chat             -> the OCTOS_SKILLS_PATH env var (colon-separated), or
#                         a `./skills/` dir in the chat cwd.
#   So this script installs PER-PROFILE by default (the path serve/gateway and
#   thus robrix actually load from).
#
# Usage:
#   bash deploy-to-octos.sh [PROFILE_ID] [DATA_DIR]
#     PROFILE_ID   gateway/serve profile id (default: feetech).
#                  Installs into <DATA_DIR>/profiles/<PROFILE_ID>/data/skills/.
#     DATA_DIR     octos data dir = the --data-dir you run `octos serve` with
#                  (default: ~/.octos). On Linux this is also $OCTOS_HOME.
#
# After it runs: (re)start `octos serve --data-dir <DATA_DIR>` so the profile's
# gateway registers the tools, and make sure OMNI_CAR_PORT + serial perms are
# set (see the printed next steps).
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROFILE_ID="${1:-feetech}"
DATA_DIR="${2:-$HOME/.octos}"
SKILL_NAME="feetech-omni-car"
DST="$DATA_DIR/profiles/$PROFILE_ID/data/skills/$SKILL_NAME"

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

# Detect the platform default serial device name for the hints below.
case "$(uname -s)" in
  Darwin) PORT_HINT="/dev/cu.usbmodem*  (run: ls /dev/cu.usbmodem* )" ;;
  *)      PORT_HINT="/dev/ttyACM0" ;;
esac

cat <<NOTE

[deploy] next steps
-------------------
1. Serial port (OMNI_CAR_PORT):
     - Linux: /dev/ttyACM0 (skill default). Grant access:
         sudo usermod -aG dialout \$USER     # then re-login
     - macOS: there is NO /dev/ttyACM0; the adapter shows up as
         /dev/cu.usbmodem<XXXX>             # find it with: ls /dev/cu.usbmodem*
       You MUST set OMNI_CAR_PORT to the real device on macOS.
   This box looks like it should use: $PORT_HINT

2. Smoke-test the binary directly (no octos), with the car powered + on a SAFE
   spot (wheels clear). Replace the port with yours:
     echo '{}' | OMNI_CAR_PORT=$PORT_HINT "$DST/main" init_motors
     echo '{"vx":0,"vy":0,"omega":300,"duration_secs":2}' | OMNI_CAR_PORT=... "$DST/main" move_base
     echo '{}' | "$DST/main" robot_estop
   Each prints {"output":...,"success":true} and the car should react.

3. Tell the profile where the serial port is. Either put it in the profile JSON
   (recommended — see feetech.profile.example.json):
       "config": { "env_vars": { "OMNI_CAR_PORT": "/dev/ttyACM0" }, ... }
   or export it in the serve process environment (it propagates to the plugin):
       export OMNI_CAR_PORT=/dev/ttyACM0

4. Start the gateway/serve (registers the 4 tools + the Matrix appservice):
     OMNI_CAR_PORT=... octos serve --data-dir "$DATA_DIR" --port 8010
   Look for:  "loaded plugin tools tools=4 profile=$PROFILE_ID"

   Quick CLI check WITHOUT a profile (chat uses OCTOS_SKILLS_PATH, not profiles):
     OCTOS_SKILLS_PATH="$DST/.." OMNI_CAR_PORT=... \\
       octos chat --provider moonshot --model kimi-k2.5 -v -m "原地旋转2秒"

5. From robrix: message the bot "原地逆时针旋转3秒" — it sequences
   init_motors -> move_base{omega,duration_secs:3} (auto-stops). Say "急停" to estop.
NOTE
