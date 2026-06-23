# Drive the Feetech omni-car from robrix (Matrix) — deploy guide

End-to-end: **robrix (Matrix GUI) → palpo (homeserver) → octos appservice →
agent (LLM) → this skill's binary → serial → the real car.**

This skill is a **binary-protocol** octos skill (no HTTP, no dora). octos runs
the skill binary (installed as `main`) — `main <tool>` per tool call, args JSON
on stdin. So the **car's USB-TTL must be on the same machine that runs the octos
gateway** (octos spawns the binary locally). dora / dora-rs are NOT needed.

> Verified working on macOS 2026-06-23 (robrix → octos → real car, chassis
> rotate 3s). The same steps apply on Linux; the only difference is the serial
> device name (see §1).

## 0. Prerequisites (on the box with the car)

- The 3-servo car wired (IDs 13/14/15), USB-TTL plugged in.
- Rust toolchain (to build the bridge).
- `octos` built **with the `api` feature** (needed for `serve`/Matrix):
  `cargo install --path crates/octos-cli --features "api,…"` (or a debug build).
- An LLM key the profile uses (e.g. `MOONSHOT_API_KEY` in your env).
- A reachable palpo homeserver (this box, another host, or the
  `robrix2/palpo-and-octos-deploy` compose). robrix connects to palpo's CS-API.

## 1. Serial device name (Linux vs macOS) — read this first

The skill defaults to `/dev/ttyACM0`, a **Linux** name. Set `OMNI_CAR_PORT`
to the real device:

- **Linux:** usually `/dev/ttyACM0`. Grant access: `sudo usermod -aG dialout $USER` (re-login).
- **macOS:** there is **no `/dev/ttyACM0`**. The adapter appears as
  `/dev/cu.usbmodem<XXXX>` — find it with `ls /dev/cu.usbmodem*`. You **must**
  set `OMNI_CAR_PORT` to that path.

## 2. Build + install the skill (PER-PROFILE)

> octos 1.1.0: the legacy global `<data-dir>/skills/` is **no longer scanned**.
> gateway/serve load skills **per-profile** from
> `<data-dir>/profiles/<id>/data/skills/`. The deploy script installs there.

```bash
cd octos-and-skills
bash deploy-to-octos.sh feetech "$HOME/.octos"
# -> installs into ~/.octos/profiles/feetech/data/skills/feetech-omni-car/ (binary as `main`)
```

Smoke-test the binary WITHOUT octos (car powered, wheels clear). Use YOUR port:

```bash
SKILL=~/.octos/profiles/feetech/data/skills/feetech-omni-car
PORT=/dev/ttyACM0   # macOS: /dev/cu.usbmodem<XXXX>
echo '{}'                                            | OMNI_CAR_PORT=$PORT "$SKILL/main" init_motors
echo '{"vx":0,"vy":0,"omega":300,"duration_secs":2}' | OMNI_CAR_PORT=$PORT "$SKILL/main" move_base
echo '{}'                                            | "$SKILL/main" robot_estop
```

Each prints `{"output":...,"success":true}` and the car should react. If this
works, octos will too.

## 3. octos Matrix profile (a gateway/serve UserProfile)

Copy `feetech.profile.example.json` to `$OCTOS_HOME/profiles/feetech.json` and
fill in the placeholders. Key points (all learned the hard way):

- **`config.env_vars.OMNI_CAR_PORT`** — set the serial device here so the plugin
  process opens the right port. (You can also export it in the serve env.)
- **`homeserver`** — palpo's CS-API reachable **from the host running octos**.
  With palpo in Docker and octos on the host, use the host port mapping
  (e.g. `http://127.0.0.1:8128`), **NOT** the docker-internal `http://palpo:8008`.
- **`as_token`/`hs_token`/`server_name`** — MUST match palpo's appservice
  registration (`appservices/octos-registration.yaml`).
- **`created_at`/`updated_at`** — REQUIRED, octos skips profiles missing these.

```jsonc
{
  "id": "feetech", "name": "Feetech Omni Car", "enabled": true,
  "created_at": "2025-01-01T00:00:00Z", "updated_at": "2025-01-01T00:00:00Z",
  "config": {
    "llm": { "primary": { "family_id": "moonshot", "model_id": "kimi-k2.5",
                          "route": { "api_key_env": "MOONSHOT_API_KEY" } } },
    "admin_mode": true,
    "env_vars": { "OMNI_CAR_PORT": "/dev/ttyACM0" },   // macOS: /dev/cu.usbmodem<XXXX>
    "channels": [
      {
        "type": "matrix",
        "homeserver": "http://127.0.0.1:8128",          // host-reachable palpo CS-API
        "as_token": "<MATCH octos-registration.yaml>",
        "hs_token": "<MATCH octos-registration.yaml>",
        "server_name": "127.0.0.1:8128",
        "sender_localpart": "octosbot", "user_prefix": "octosbot_",
        "port": 8009, "allowed_senders": []
      }
    ],
    "gateway": { "max_history": 50, "queue_mode": "followup" }
  }
}
```

> **Two different "profile" concepts — don't confuse them:**
> `octos chat --profile coding|swarm` is the *runtime* profile (tool-policy
> presets); it does NOT carry LLM/channels/skills. The multi-tenant
> **UserProfile** above (flat `profiles/<id>.json` + `profiles/<id>/data/`) is
> what `serve`/`gateway` load. Matrix + skills require this one.

## 4. Run octos (registers the skill + the Matrix appservice)

```bash
export MOONSHOT_API_KEY=...                 # key the profile references
OMNI_CAR_PORT=/dev/ttyACM0 \                # macOS: /dev/cu.usbmodem<XXXX>
  octos serve --data-dir "$HOME/.octos" --host 127.0.0.1 --port 8010
```

- `serve` reads profiles from `<--data-dir>/profiles/`, spawns one gateway child
  per enabled profile; that gateway loads the per-profile skills dir → registers
  `init_motors / move_base / stop / robot_estop` and starts the Matrix
  appservice on `:8009`.
- palpo must reach this appservice at the `url` in `octos-registration.yaml`
  (e.g. `http://host.docker.internal:8009` when palpo is dockerized, octos on host).

Confirm in the gateway log (`<data-dir>/logs/serve.<date>.log`):
- `loaded plugin tools tools=4 profile=feetech`
- `Matrix appservice listening on 0.0.0.0:8009 profile=feetech`

Sanity-check the appservice↔palpo link:
```bash
curl -s -H "Authorization: Bearer <as_token>" \
  "http://127.0.0.1:8128/_matrix/client/v3/account/whoami?user_id=@octosbot:127.0.0.1:8128"
# -> {"user_id":"@octosbot:127.0.0.1:8128","device_id":"appservice"}
```

### Quick CLI check without Matrix (optional)

`octos chat` does NOT use gateway profiles; point it at the skill dir with
`OCTOS_SKILLS_PATH` (the parent of the `feetech-omni-car/` dir):

```bash
OCTOS_SKILLS_PATH=~/.octos/profiles/feetech/data/skills \
OMNI_CAR_PORT=/dev/ttyACM0 \
  octos chat --provider moonshot --model kimi-k2.5 -v -m "原地逆时针旋转2秒"
```

## 5. From robrix

Log in (palpo homeserver), open/create a room, invite `@octosbot:<server_name>`
(prefer a fresh room so stale context doesn't confuse the agent), then message:
- "原地逆时针旋转3秒" → `init_motors` → `move_base{omega:…,duration_secs:3}` (auto-stops).
- "让小车前进2秒"     → `init_motors` → `move_base{vx:…,duration_secs:2}`.
- "停车" / "急停"     → `stop` / `robot_estop`.

## Notes / gotchas

- **Safety stop is the `robot_estop` TOOL** (the agent calls it) and the
  SKILL.md `emergency_shutdown` hook (`./main robot_estop`). The Matrix `/estop`
  *slash bypass* built elsewhere targets a SPEC HTTP bridge (`:8769`) — this
  binary-protocol skill has none, so `/estop` slash does NOT apply; say
  "急停"/"停车" and let the agent call the tool. Physical power-off is the
  ultimate stop.
- `init_motors` must run once after power-on (and again after any `robot_estop`).
  The manifest/SKILL.md tell the agent this.
- Speed is internal ticks ±1500 (1 tick ≈ 0.000077 m/s); `move_base` with
  `duration_secs` auto-stops.
- Everything (octos + skill binary + serial) is on this one host; palpo/robrix
  may be elsewhere on the network.
