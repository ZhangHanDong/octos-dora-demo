# feetech-omni-car

飞特舵机三轮全向底盘 octos skill。通过自然语言指令控制小车运动，无需编写代码。
二进制协议 plugin（不走 HTTP/dora）：octos 每次工具调用拉起可执行文件 `main <tool>`，
args JSON 走 stdin，开串口→执行→退出。

**端到端 robrix(Matrix)→octos→真车** 的完整部署见 **[ROBRIX-DEPLOY.md](ROBRIX-DEPLOY.md)**，
profile 模板见 **[feetech.profile.example.json](feetech.profile.example.json)**。

## 文件结构

```
feetech-omni-car/
├── SKILL.md                       # octos skill 描述（含 preflight / emergency_shutdown 钩子）
├── manifest.json                  # 工具清单（"binary":"main"）
├── bridge/                        # plugin 源码（Rust）；编译产物 omni-car-bridge 未入库（.gitignore）
├── deploy-to-octos.sh             # 编译 + 按 profile 安装（产物改名为 main）
├── feetech.profile.example.json   # gateway/serve 用的 Matrix profile 模板
├── ROBRIX-DEPLOY.md               # robrix→octos→真车 全套部署
└── README.md                      # 本文档
```

## 硬件要求

- 飞特 STS/SMS 系列串口总线舵机 × 3，ID 分别设为 13、14、15，以 120° 均布
- USB 转 TTL 串口适配器
  - **Linux**：通常 `/dev/ttyACM0`（skill 默认值）
  - **macOS**：没有 `/dev/ttyACM0`，是 `/dev/cu.usbmodem<XXXX>`（`ls /dev/cu.usbmodem*` 查），
    必须用 `OMNI_CAR_PORT` 覆盖
- 舵机供电 6V~12V（独立于 USB 供电）

## 安装（octos 1.1.0：按 profile）

> octos 1.1.0 起，全局 `<data-dir>/skills/` **不再被扫描**。gateway/serve 按 profile
> 从 `<data-dir>/profiles/<id>/data/skills/` 加载；`octos chat` 用 `OCTOS_SKILLS_PATH`。

```bash
cd octos-and-skills
bash deploy-to-octos.sh feetech "$HOME/.octos"
# 编译 bridge 并装入 ~/.octos/profiles/feetech/data/skills/feetech-omni-car/（二进制名 main）
```

## 自然语言控制（快速 CLI 验证）

`octos chat` 不读 gateway profile，用 `OCTOS_SKILLS_PATH` 指向 skill 父目录：

```bash
OCTOS_SKILLS_PATH=~/.octos/profiles/feetech/data/skills \
OMNI_CAR_PORT=/dev/ttyACM0 \    # macOS: /dev/cu.usbmodem<XXXX>
  octos chat --provider moonshot --model kimi-k2.5 -v -m "让小车原地旋转3秒后停止"
```

octos 会自动依次调用 `init_motors → move_base → stop` 完成整个任务。
要从 robrix(Matrix) 驱动，见 [ROBRIX-DEPLOY.md](ROBRIX-DEPLOY.md)。

## 工具参数说明

| 工具 | 说明 | 参数 | 类型 | 范围 | 参数说明 |
|------|------|------|------|------|---------|
| `init_motors` | 初始化电机，切换到轮式模式并使能力矩。**每次上电后必须首先调用** | — | — | — | — |
| `move_base` | 运动控制 | `vx` | number | -1500 ~ 1500 | 前后速度，正=前进，负=后退 |
| | | `vy` | number | -1500 ~ 1500 | 左右平移，正=左，负=右 |
| | | `omega` | number | -1500 ~ 1500 | 旋转速度，正=逆时针，负=顺时针 |
| | | `duration_secs` | number（可选） | 0 ~ 30 | 运动持续秒数，到时自动停止；不传则持续运动直到下一条指令 |
| `stop` | 所有轮速归零（软停，力矩保持） | — | — | — | — |
| `robot_estop` | 紧急停止，立即归零速度并关闭力矩。执行后需重新调用 `init_motors` | `reason` | string | — | 停止原因（可选，仅用于日志） |

速度单位为飞特舵机内部刻度，三轴混合后每轮独立裁剪至 ±1500。

**刻度与实际线速度对应关系**（基于轮径 10 cm 实测，omega 纯旋转模式下测得）：

| 速度刻度 | 近似线速度 |
|---------|-----------|
| 300 | ~0.023 m/s |
| 500 | ~0.038 m/s |
| 800 | ~0.061 m/s |
| 1000 | ~0.076 m/s |
| 1200 | ~0.091 m/s |
| 1500 | ~0.114 m/s（最大） |

换算关系：**1 刻度 ≈ 0.000077 m/s**（线性）。多轴混合运动时实际速度取决于合成后的轮速，与单轴有偏差。

## 自然语言指令示例

| 自然语言指令 | 实际工具调用 |
|-------------|------------|
| 让小车前进 | `move_base vx=800, vy=0, omega=0` |
| 让小车后退 | `move_base vx=-800, vy=0, omega=0` |
| 让小车向左平移 | `move_base vx=0, vy=500, omega=0` |
| 让小车向右平移 | `move_base vx=0, vy=-500, omega=0` |
| 让小车原地逆时针旋转 | `move_base vx=0, vy=0, omega=500` |
| 让小车原地顺时针旋转 | `move_base vx=0, vy=0, omega=-500` |
| 让小车斜向左前方运动 | `move_base vx=500, vy=400, omega=0` |
| 让小车前进3秒后停止 | `move_base vx=800, vy=0, omega=0, duration_secs=3` |
| 让小车原地旋转5秒后停止 | `move_base vx=0, vy=0, omega=500, duration_secs=5` |
| 以500的速度向左平移2秒 | `move_base vx=0, vy=500, omega=0, duration_secs=2` |
| 停止小车 | `stop` |
| 紧急停止 | `robot_estop` |

## 常用动作速度参考

| 动作 | vx | vy | omega | 近似线速度 |
|------|----|----|-------|-----------|
| 前进（慢） | 500 | 0 | 0 | ~0.038 m/s |
| 前进（快） | 1200 | 0 | 0 | ~0.091 m/s |
| 后退 | -800 | 0 | 0 | ~0.061 m/s |
| 左平移 | 0 | 500 | 0 | ~0.038 m/s |
| 右平移 | 0 | -500 | 0 | ~0.038 m/s |
| 逆时针旋转（慢） | 0 | 0 | 300 | ~0.023 m/s |
| 逆时针旋转（快） | 0 | 0 | 800 | ~0.061 m/s |
| 顺时针旋转 | 0 | 0 | -500 | ~0.038 m/s |
| 左前斜向 | 400 | 400 | 0 | — |

## 环境变量

| 变量 | 默认值 | 说明 |
|------|--------|------|
| `OMNI_CAR_PORT` | `/dev/ttyACM0` | 串口设备路径（Linux）。**macOS 用 `/dev/cu.usbmodem<XXXX>`** |
| `OMNI_CAR_BAUD` | `1000000` | 波特率（通常无需修改） |

gateway/serve 下，把 `OMNI_CAR_PORT` 放进 profile 的 `config.env_vars`（见
[feetech.profile.example.json](feetech.profile.example.json)），插件子进程会继承。

## 重新编译 / 重新安装 plugin

直接重跑部署脚本（编译 + 装入对应 profile，二进制改名为 `main`）：

```bash
bash deploy-to-octos.sh feetech "$HOME/.octos"
```

或手动编译：`cd bridge && cargo build --release`，产物在
`bridge/target/release/omni-car-bridge`（安装时需改名为 `main`）。

## 注意事项

- `robot_estop` 执行后舵机力矩关闭，重新运动前必须再次调用 `init_motors`
- `stop` 只是速度归零，力矩保持输出（舵机仍锁定）
- 大速度多轴组合时（如 vx=1500, vy=1500），混合后的轮速会被裁剪，实际航向与预期有偏差
