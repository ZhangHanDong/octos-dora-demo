---
name: feetech-omni-car
description: 控制飞特舵机三轮全向底盘小车的移动、旋转和停车。触发关键词：nano、移动、旋转、前进、后退、平移
version: 0.2.0
author: dorarobotics
robot_type: feetech-omni-car
required_safety_tier: safe_motion
hardware_requirements: /dev/ttyACM0 (or set OMNI_CAR_PORT), 3x Feetech STS/SMS servos (ID 13/14/15)
preflight:
  - label: check serial port exists
    command: bash -c 'test -e "${OMNI_CAR_PORT:-/dev/ttyACM0}"'
    timeout_secs: 5
    critical: true
init:
  - label: start omni-car bridge
    command: /home/dora/.octos/skills/skills/feetech-omni-car/omni-car-bridge
    timeout_secs: 10
    critical: true
ready_check:
  - label: bridge HTTP responds
    command: curl -fsS -m 2 http://127.0.0.1:8770/healthz
    timeout_secs: 3
    retries: 10
    critical: true
shutdown:
  - label: stop bridge process
    command: pkill -f omni_car_bridge || true
    timeout_secs: 5
    critical: false
emergency_shutdown:
  - label: estop via bridge
    command: curl -fsS -X POST http://127.0.0.1:8770/tools/robot_estop -H "Content-Type: application/json" -d '{"args":{}}'
    timeout_secs: 5
    critical: true
---

# Feetech Omni-Wheel Car（飞特三轮全向底盘）

三轮全向底盘，三个飞特串口总线舵机（ID 13/14/15）以 120° 均布驱动。
通过本地 HTTP bridge (`http://127.0.0.1:8770`) 控制，串口由 bridge 进程持有。

## 轮子布局

| 舵机 ID | 位置   | 安装角度 |
|--------|--------|---------|
| 13     | 左前轮 | 60°     |
| 14     | 后轮   | 180°    |
| 15     | 右前轮 | 300°    |

## 使用前置条件

**必须先调用 `init_motors` 初始化**，将舵机切换为恒速模式（Wheel Mode）。

## 运动坐标系

- `vx`：前后速度（正 = 前进，负 = 后退），范围 -1500 ~ 1500（内部刻度）
- `vy`：左右速度（正 = 左平移，负 = 右平移）
- `omega`：旋转速度（正 = 逆时针，负 = 顺时针）

注意：三轴混合后每轮速度在 ±1500 内裁剪。大速度组合时实际航向可能偏离预期。

## 常见操作示例

```
前进：      POST /tools/move_base  {"args": {"vx": 1000, "vy": 0,    "omega": 0}}
后退：      POST /tools/move_base  {"args": {"vx": -500, "vy": 0,    "omega": 0}}
左平移：    POST /tools/move_base  {"args": {"vx": 0,    "vy": 500,  "omega": 0}}
右平移：    POST /tools/move_base  {"args": {"vx": 0,    "vy": -500, "omega": 0}}
原地旋转：  POST /tools/move_base  {"args": {"vx": 0,    "vy": 0,    "omega": 300}}
停车：      POST /tools/stop       {"args": {}}
紧急停止：  POST /tools/robot_estop {"args": {}}
```

## 工具调用顺序

1. `init_motors` — 上电后必须先调用一次（bridge 已启动后调用）
2. `move_base` / `stop` — 反复调用控制运动
3. `robot.estop` — 任何安全顾虑时立即调用

## 错误码

| Code | 含义 | 处理方式 |
|------|------|---------|
| `INIT_ERROR` | 舵机初始化失败（超时/校验和错误） | 检查电源和串口连接 |
| `NOT_INITIALIZED` | 未调用 init_motors | 先调用 init_motors |
| `MOTOR_ERROR` | 运动指令写入失败 | 检查总线连接，重试 |
| `INVALID_PARAMS` | 参数格式错误 | 检查 vx/vy/omega 字段 |
| `BRIDGE_DOWN` | bridge 进程不可达 | operator 重启 bridge |

## 安全

- `required_safety_tier: safe_motion`
- `robot.estop` 会置零速度并关闭力矩，执行后需重新调用 init_motors
- 速度上限硬限制 ±1500（bridge 内部裁剪）
- bridge 持有串口连接，LLM 不接触串口路径
- 串口路径通过环境变量 `OMNI_CAR_PORT` 配置，不经过 LLM 参数传递
