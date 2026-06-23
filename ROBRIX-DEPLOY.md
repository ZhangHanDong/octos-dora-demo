# Drive the Feetech omni-car from robrix (Matrix) — Linux deploy

End-to-end: **robrix (Matrix GUI) → palpo (homeserver) → octos appservice →
agent (LLM) → this skill's binary → serial → the real car.**

This skill is a **binary-protocol** octos skill (no HTTP, no dora). octos runs
the skill binary (installed as `main`) `main <tool>` per tool call (args JSON on
stdin). So the **car's USB-TTL must be on the same machine that runs the octos
gateway** (octos spawns the binary locally). dora / dora-rs are NOT needed for
this robot.

## 0. Prerequisites (on the Linux box with the car)

- The 3-servo car wired (IDs 13/14/15), USB-TTL plugged → `/dev/ttyACM0`
  (or set `OMNI_CAR_PORT`), and serial perms: `sudo usermod -aG dialout $USER` (re-login).
- Rust toolchain (to build the bridge).
- `octos` built **with the `api` feature** (needed for `serve`/Matrix):
  `cargo install --path crates/octos-cli --features "api,…"` (or your build).
- An LLM key the profile uses (e.g. `MOONSHOT_API_KEY`).
- A reachable palpo homeserver (this box, another host, or the
  `robrix2/palpo-and-octos-deploy` compose). robrix connects to palpo's CS-API.

## 1. Build + install the skill

```bash
cd octos-and-skills
bash deploy-to-octos.sh "$HOME/.octos"     # build + install into ~/.octos/skills/feetech-omni-car
```
Then smoke-test the binary WITHOUT octos (car powered):
```bash
echo '{}' | OMNI_CAR_PORT=/dev/ttyACM0 ~/.octos/skills/feetech-omni-car/main init_motors
echo '{"vx":500,"vy":0,"omega":0,"duration_secs":2}' | OMNI_CAR_PORT=/dev/ttyACM0 ~/.octos/skills/feetech-omni-car/main move_base
echo '{}' | ~/.octos/skills/feetech-omni-car/main robot_estop
```
Each prints `{"output":...,"success":true}` and the car should move. If this
works, octos will too.

## 2. octos Matrix profile

Put a profile at `$OCTOS_HOME/profiles/botfather.json`. Tokens + `server_name`
MUST match palpo's appservice registration (`octos-registration.yaml`). Replace
the homeserver/tokens/server_name with yours.

```jsonc
{
  "id": "botfather",
  "name": "BotFather",
  "enabled": true,
  "created_at": "2025-01-01T00:00:00Z",   // REQUIRED — octos skips profiles missing these
  "updated_at": "2025-01-01T00:00:00Z",
  "config": {
    "llm": { "primary": { "family_id": "moonshot", "model_id": "kimi-k2.5",
                          "route": { "api_key_env": "MOONSHOT_API_KEY" } } },
    "admin_mode": true,
    "channels": [
      {
        "type": "matrix",
        "homeserver": "http://127.0.0.1:8128",   // palpo CS-API reachable from THIS host
        "as_token": "<MATCH octos-registration.yaml as_token>",
        "hs_token": "<MATCH octos-registration.yaml hs_token>",
        "server_name": "127.0.0.1:8128",          // palpo's server_name
        "sender_localpart": "octosbot",
        "user_prefix": "octosbot_",
        "port": 8009,
        "allowed_senders": []
      }
    ],
    "gateway": { "max_history": 50, "queue_mode": "followup" }
  }
}
```

## 3. Run octos (registers the skill + the Matrix appservice)

```bash
export MOONSHOT_API_KEY=...        # the key the profile references
export OMNI_CAR_PORT=/dev/ttyACM0  # so the skill binary finds the serial
octos serve --data-dir "$HOME/.octos" --host 127.0.0.1 --port 8010
```
- `serve` reads profiles from `<--data-dir>/profiles/` (NOT from a config file's
  `data_dir`), spawns one gateway child per enabled profile, and that gateway
  loads `<--data-dir>/skills/` → registers `init_motors / move_base / stop /
  robot_estop` and starts the Matrix appservice on `:8009`.
- palpo must be able to reach this appservice at the URL in
  `octos-registration.yaml` (e.g. `http://host.docker.internal:8009` when palpo
  is in Docker and octos on the host).

Confirm in the gateway log: `registered ... tools` for `feetech-omni-car` and
`Matrix appservice listening on 0.0.0.0:8009`.

## 4. From robrix

Log in (palpo homeserver), open/create a room, invite `@octosbot:<server_name>`,
then message it:
- "让小车前进2秒" → agent calls `init_motors` → `move_base{vx:…,duration_secs:2}`.
- "原地逆时针转3秒后停" → `move_base{omega:…,duration_secs:3}` → `stop`.
- "停车" / "急停" → `stop` / `robot_estop`.

## Notes / gotchas

- **Safety stop on this robot is the `robot_estop` TOOL** (the agent calls it),
  and the SKILL.md `emergency_shutdown` hook (`./main robot_estop`).
  The Matrix `/estop` *slash bypass* we built elsewhere targets a SPEC HTTP
  bridge (`:8769`) — this binary-protocol skill has none, so `/estop` slash does
  not apply here; say "急停"/"停车" and let the agent call the tool. Physical
  power-off remains the ultimate stop.
- `init_motors` must run once after power-on (and again after any `robot_estop`).
  The agent is told this via the manifest/SKILL.md.
- Speed is internal ticks ±1500 (1 tick ≈ 0.000077 m/s); `move_base` with
  `duration_secs` auto-stops.
- Everything (octos + skill binary + serial) is on this one host; palpo/robrix
  may be elsewhere on the network.
