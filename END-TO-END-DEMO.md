# 端到端 Demo：用 robrix(Matrix)聊天驱动飞特真车

一句话：**在手机/桌面的 Matrix 客户端 robrix 里发一句中文（"原地逆时针旋转3秒"），
经 octos agent（Moonshot LLM）理解并编排工具，最终让真实的飞特三轮全向底盘动起来。**

> 本文是整套链路的权威总览，记录了 2026-06-23/24 在 macOS 上**真机跑通**（手机端
> + 桌面端都验证）的全部步骤、踩的坑与排查方法。skill 本身的工具/参数参考见
> [README.md](README.md)，Linux 服务端部署见 [ROBRIX-DEPLOY.md](ROBRIX-DEPLOY.md)，
> profile 模板见 [feetech.profile.example.json](feetech.profile.example.json)。

## 1. 架构 / 数据流

```
[robrix on Android 手机]  ┐
                          ├─▶ palpo(Matrix homeserver, OrbStack docker)
[robrix on macOS 桌面]    ┘        │  appservice push (host.docker.internal:8009)
                                   ▼
                        octos serve ─spawns─▶ gateway(profile=feetech)
                                   │            ├─ Matrix appservice :8009  (@octosbot)
                                   │            ├─ Moonshot LLM (kimi-k2.5) 理解+编排
                                   │            └─ feetech-omni-car skill（二进制协议）
                                   ▼
                        main <tool>  (init_motors / move_base / stop / robot_estop)
                          args JSON → stdin，开串口→执行→退出
                                   ▼
                        USB-TTL 串口(OMNI_CAR_PORT) ─▶ 3× 飞特总线舵机 ─▶ 小车
```

- skill 是 **octos 二进制协议 plugin**（不走 HTTP、不走 dora）：octos 每次工具调用
  拉起 `main <tool_name>`，args 作为裸 JSON 对象写 stdin，回 `{"output":..,"success":bool}`。
- 因此 **octos 网关、skill 二进制、车的 USB 串口必须在同一台机器**。手机/桌面 robrix
  只是 Matrix 前端，可以在别处。

## 2. 组件与端口（本次 macOS 实测值）

| 组件 | 地址/值 | 说明 |
|------|---------|------|
| palpo homeserver | `127.0.0.1:8128`（host 映射 → 容器 8008） | OrbStack docker；**只发布在 loopback** |
| palpo server_name | `127.0.0.1:8128` | 用户 ID 形如 `@alex:127.0.0.1:8128` |
| octos serve REST/dashboard | `127.0.0.1:8010` | `--port 8010` |
| octos Matrix appservice | `:8009`（`@octosbot`） | palpo 推事件到 `host.docker.internal:8009` |
| octos 二进制 | `octos/target/debug/octos`（1.1.0，**带 api feature**） | `serve` 需要 api |
| profile | `~/.octos/profiles/feetech.json` | gateway/serve 的 UserProfile |
| skill 安装目录 | `~/.octos/profiles/feetech/data/skills/feetech-omni-car/` | **per-profile**（见下） |
| 串口 | macOS `/dev/cu.usbmodem<XXXX>`；Linux `/dev/ttyACM0` | `OMNI_CAR_PORT` 覆盖 |
| LLM | Moonshot `kimi-k2.5`，`MOONSHOT_API_KEY` | profile `llm.primary` |
| robrix 桌面 | `robrix2/target/release/robrix` | 自动恢复会话 |
| robrix 安卓 | `adb install` 的 apk | 见 §5 |
| 本地 dev 账号 | `alex` / `demo`（密码为本地测试值） | 经 palpo 登录验证可用 |

## 3. octos 1.1.0 的关键事实（踩坑结论）

1. **skill 加载是 per-profile**：全局 `~/.octos/skills/` **已废弃不再扫**。
   - gateway/serve：`<data-dir>/profiles/<id>/data/skills/<skill>/`
   - `octos chat`：`OCTOS_SKILLS_PATH`（冒号分隔）或 cwd 的 `./skills`
2. **可执行文件名**：octos 按候选 `[manifest.name, dir_name, "main"]` 找，**不读
   manifest 的 `binary` 字段**。→ 二进制装成 **`main`** 最稳。
3. **stdin 是裸 args 对象**（无 `{"args":..}` 包裹）；argv[1] 是 tool 名。
4. **两种 "profile" 别混**：`chat --profile coding|swarm` 是 runtime 预设（与
   LLM/channel/skill 无关）；gateway/serve 用的是 ProfileStore 的 **UserProfile**
   （`profiles/<id>.json` + `profiles/<id>/data/`，带 llm/channels/env_vars）。
5. **OMNI_CAR_PORT 注入网关**：放 profile `config.env_vars`（+ serve 进程 env 双保险），
   插件子进程继承。

## 4. 服务端搭建（macOS）

```bash
# 4.1 编译 + 按 profile 安装 skill（产物改名 main）
cd octos-and-skills
bash deploy-to-octos.sh feetech "$HOME/.octos"

# 4.2 放置 profile（含 matrix channel + env_vars.OMNI_CAR_PORT）
#     拷贝 feetech.profile.example.json 到 ~/.octos/profiles/feetech.json，
#     填入与 palpo appservice 注册一致的 as_token/hs_token，
#     homeserver 用 host 可达地址 http://127.0.0.1:8128（不是 docker 内部 palpo:8008），
#     macOS 把 env_vars.OMNI_CAR_PORT 设成 /dev/cu.usbmodem<XXXX>（ls /dev/cu.usbmodem*）

# 4.3 起 serve（自动起 feetech 网关 → 注册 4 工具 + Matrix appservice :8009）
export MOONSHOT_API_KEY=...
OCTOS_SKILLS_PATH="$HOME/.octos/skills" \
OMNI_CAR_PORT=/dev/cu.usbmodem<XXXX> \
  octos serve --data-dir "$HOME/.octos" --host 127.0.0.1 --port 8010
```

确认网关日志（`~/.octos/logs/serve.<date>.log`）：
`loaded plugin tools tools=4 profile=feetech` 与
`Matrix appservice listening on 0.0.0.0:8009 profile=feetech`。

自检 appservice↔palpo：
```bash
curl -s -H "Authorization: Bearer <as_token>" \
  "http://127.0.0.1:8128/_matrix/client/v3/account/whoami?user_id=@octosbot:127.0.0.1:8128"
# -> {"user_id":"@octosbot:127.0.0.1:8128","device_id":"appservice"}
```

## 5. 手机端（Android robrix）

### 5.1 装 apk

```bash
brew install android-platform-tools     # 若没有 adb
adb devices                              # 手机开「USB 调试」并授权后应列出设备
adb install -r ~/Downloads/robrix.apk
```
**小米/红米（MIUI/HyperOS）专属坑**：开发者选项里必须额外打开 **「USB 安装」**，
否则报 `INSTALL_FAILED_USER_RESTRICTED: Install canceled by user`（常需插 SIM +
登录小米账号 + 联网才能开这个开关）；安装时盯手机屏幕点「继续安装」。

### 5.2 让手机连到 palpo —— 用 USB 反向隧道（关键）

**坑**：palpo 由 OrbStack 发布，**只在 Mac 的 `127.0.0.1` 可达**；且本测试 WiFi 有
**设备隔离**（Mac 连自己的局域网 IP 都超时），所以"同一局域网 + Mac IP"这条路走不通。

**解法**：手机本来就 USB 连着，用 `adb reverse` 走 USB 隧道，**无需同一局域网**，
而且地址正好是 `127.0.0.1:8128`，与 palpo 的 server_name 完全一致（省掉地址不匹配）：

```bash
adb reverse tcp:8128 tcp:8128      # 手机 localhost:8128 → (USB) → Mac palpo
adb reverse --list                 # 确认：UsbFfs tcp:8128 tcp:8128
# 验证（从手机内部）：
adb shell 'curl -s http://127.0.0.1:8128/_matrix/client/versions' | head -c 80
```
> USB 线不能拔；拔了/重插要重跑 `adb reverse tcp:8128 tcp:8128`。
> （若 WiFi 没有设备隔离，也可改用"同一局域网 + http://<Mac局域网IP>:port"，
> 但要先把 palpo 暴露到局域网，比如再加一个监听 0.0.0.0 的 TCP 转发到 127.0.0.1:8128。）

### 5.3 robrix 登录

- Homeserver：`http://127.0.0.1:8128`
- 用户名/密码：本地 dev 账号（如 `demo` / 本地密码）。
  > 桌面 robrix 若已登 `alex`，手机建议用**另一个账号**（如 `demo`）避免同账号多端互相影响。

## 6. 跑一次

robrix 里进入有 `@octosbot:127.0.0.1:8128` 的房间（建议新建一个干净房间并邀请它），发：
- **"原地逆时针旋转3秒"** → agent 调 `init_motors` → `move_base{omega:…, duration_secs:3}`（到时自动停）
- "让小车前进2秒" → `init_motors` → `move_base{vx:…, duration_secs:2}`
- **"急停" / "停车"** → `robot_estop` / `stop`（robot_estop 后需重新 init_motors）

## 7. 无硬件测试模式

拔掉车的 USB 后，**除"真正转轮子"外整条链照常可测**：串口打开失败返回
`{"success":false,"output":"failed to open ...usbmodem..."}` —— 这是**预期正确行为**。
判读：看到 `failed to open ...` = ✅ 链路通到了串口那步；若 `move_base/init_motors`
**根本没被调用**或 LLM 说找不到工具 = ❌ 真问题。

快速 CLI 验证（不经 Matrix）：
```bash
OCTOS_SKILLS_PATH=~/.octos/profiles/feetech/data/skills \
OMNI_CAR_PORT=/dev/cu.usbmodem<XXXX> \
  octos chat --provider moonshot --model kimi-k2.5 -v -m "原地逆时针旋转2秒"
```

## 8. 本次验证证据（实测）

- `octos chat`：`loaded plugin tools tools=4` → LLM `tool_calls=2 tool_names=init_motors, move_base`
  → `spawning plugin process ... executable=.../main` → 真车动作（桌面 robrix 实测旋转3秒）。
- 网关：`loaded plugin tools tools=4 profile=feetech` + `Matrix appservice listening on 0.0.0.0:8009`。
- appservice：`whoami` 返回 `@octosbot:127.0.0.1:8128`。
- 手机：经 `adb reverse` 从手机内部取到 palpo `/versions`（含 `org.matrix.msc3575` sliding sync）。
- 登录：palpo 对 `alex`/`demo` 密码返回 access_token。

## 9. 故障排查速查

| 现象 | 原因 / 解决 |
|------|------------|
| chat 工具里没有 init_motors 等（38 个通用工具） | skill 没加载：用 `OCTOS_SKILLS_PATH`（chat）或装到 per-profile 目录（serve） |
| `failed to open ...ttyACM0`（macOS） | macOS 无 ttyACM0；`OMNI_CAR_PORT` 改 `/dev/cu.usbmodem<XXXX>` |
| `INSTALL_FAILED_USER_RESTRICTED` | 小米没开「USB 安装」 |
| 手机连不上 palpo（同局域网也不行） | OrbStack 只 loopback + WiFi 设备隔离 → 用 `adb reverse`（§5.2） |
| `adb devices` 空/`unauthorized` | 手机弹「允许 USB 调试」点允许；换 MTP 模式、重插线 |
| 登录 403 M_FORBIDDEN | 账号/密码不对；用 §8 的 curl login 先验证 |
| 网关无 `loaded plugin tools` | profile 缺 `created_at/updated_at` 被跳过；或 skill 目录不对 |

## 10. 安全

- 急停是 `robot_estop` **工具**（agent 调用）+ SKILL.md 的 `emergency_shutdown` 钩子
  （`./main robot_estop`）。Matrix `/estop` slash 旁路针对的是别处的 HTTP 桥（:8769），
  本二进制 skill 没有，说"急停"让 agent 调工具即可。**物理断电是终极急停。**
- 真车会真的动——发运动指令前确保车在安全位置（轮子离地或周围空旷）。
