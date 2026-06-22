//! Feetech Omni-Car octos Plugin
//!
//! octos plugin 协议: binary <tool_name>，参数 JSON 从 stdin 读入，结果 JSON 输出到 stdout。
//!
//! 输入: 命令行 args[1] = tool name，stdin = JSON 参数，如 {} 或 {"vx":0,"vy":0,"omega":300}
//! 输出: {"output": "...", "success": true/false}
//!
//! 环境变量:
//!   OMNI_CAR_PORT  — 串口路径，默认 /dev/ttyACM0
//!   OMNI_CAR_BAUD  — 波特率，默认 1000000

use bytes::BufMut;
use serde_json::{Value, json};
use std::{env, io::{self, Read}};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::Duration;
use tokio_serial::SerialPortBuilderExt;

// --- 硬件常量 ---

const WHEEL_IDS: [u8; 3] = [13, 14, 15];
const ADDR_LOCK: u8 = 55;
const ADDR_MODE: u8 = 33;
const ADDR_TORQUE: u8 = 40;
const ADDR_SPEED: u8 = 46;
const SPEED_LIMIT: f32 = 1500.0;

// --- SCS 协议 ---

fn checksum(id: u8, length: u8, instruction: u8, params: &[u8]) -> u8 {
    let sum: u32 = id as u32 + length as u32 + instruction as u32
        + params.iter().map(|&b| b as u32).sum::<u32>();
    !(sum as u8)
}

fn pack(id: u8, instruction: u8, params: &[u8]) -> Vec<u8> {
    let length = (params.len() + 2) as u8;
    let mut buf = Vec::with_capacity(params.len() + 6);
    buf.put_u8(0xFF);
    buf.put_u8(0xFF);
    buf.put_u8(id);
    buf.put_u8(length);
    buf.put_u8(instruction);
    buf.extend_from_slice(params);
    buf.put_u8(checksum(id, length, instruction, params));
    buf
}

// --- 串口总线 ---

struct Bus {
    stream: tokio_serial::SerialStream,
    read_timeout: Duration,
}

impl Bus {
    fn open(port: &str, baud: u32) -> anyhow::Result<Self> {
        let stream = tokio_serial::new(port, baud)
            .timeout(Duration::from_millis(100))
            .open_native_async()?;
        Ok(Self { stream, read_timeout: Duration::from_millis(20) })
    }

    async fn write_byte(&mut self, id: u8, addr: u8, value: u8) -> anyhow::Result<()> {
        let packet = pack(id, 0x03, &[addr, value]);
        self.transfer(&packet, 6).await?;
        Ok(())
    }

    async fn write_word(&mut self, id: u8, addr: u8, value: u16) -> anyhow::Result<()> {
        let [lo, hi] = value.to_le_bytes();
        let packet = pack(id, 0x03, &[addr, lo, hi]);
        self.stream.write_all(&packet).await?;
        Ok(())
    }

    async fn transfer(&mut self, packet: &[u8], response_len: usize) -> anyhow::Result<Vec<u8>> {
        let mut discard = [0u8; 64];
        while let Ok(Ok(n)) =
            tokio::time::timeout(Duration::from_millis(2), AsyncReadExt::read(&mut self.stream, &mut discard)).await
        {
            if n == 0 { break; }
        }
        self.stream.write_all(packet).await?;
        let mut buf = vec![0u8; response_len];
        tokio::time::timeout(self.read_timeout, AsyncReadExt::read_exact(&mut self.stream, &mut buf))
            .await
            .map_err(|_| anyhow::anyhow!("read timeout"))?
            .map_err(|e| anyhow::anyhow!("read error: {e}"))?;
        if buf.len() < 6 || buf[0] != 0xFF || buf[1] != 0xFF {
            anyhow::bail!("invalid response header");
        }
        Ok(buf)
    }
}

// --- 底盘控制 ---

async fn init_motors(bus: &mut Bus) -> anyhow::Result<()> {
    for &id in &WHEEL_IDS {
        bus.write_byte(id, ADDR_LOCK, 0).await
            .map_err(|e| anyhow::anyhow!("unlock failed id={id}: {e}"))?;
        bus.write_byte(id, ADDR_MODE, 1).await
            .map_err(|e| anyhow::anyhow!("set mode failed id={id}: {e}"))?;
        bus.write_byte(id, ADDR_LOCK, 1).await
            .map_err(|e| anyhow::anyhow!("relock failed id={id}: {e}"))?;
        bus.write_byte(id, ADDR_TORQUE, 1).await
            .map_err(|e| anyhow::anyhow!("enable torque failed id={id}: {e}"))?;
    }
    Ok(())
}

async fn set_wheel_speed(bus: &mut Bus, id: u8, speed: f32) -> anyhow::Result<()> {
    let clamped = speed.clamp(-SPEED_LIMIT, SPEED_LIMIT);
    let mag = clamped.abs() as u16;
    let reg = if clamped < 0.0 { mag | 0x8000 } else { mag };
    bus.write_word(id, ADDR_SPEED, reg).await
}

async fn apply_velocity(bus: &mut Bus, vx: f32, vy: f32, omega: f32) -> anyhow::Result<()> {
    let v_left  = -0.866 * vx + 0.5 * vy + omega;
    let v_back  =  0.0   * vx - 1.0 * vy + omega;
    let v_right =  0.866 * vx + 0.5 * vy + omega;
    set_wheel_speed(bus, WHEEL_IDS[0], v_left).await?;
    set_wheel_speed(bus, WHEEL_IDS[1], v_back).await?;
    set_wheel_speed(bus, WHEEL_IDS[2], v_right).await?;
    Ok(())
}

async fn estop(bus: &mut Bus) {
    for &id in &WHEEL_IDS {
        let _ = bus.write_word(id, ADDR_SPEED, 0).await;
        let _ = bus.write_byte(id, ADDR_TORQUE, 0).await;
    }
}

// --- Plugin 协议 ---
// octos 调用方式: binary <tool_name>
// stdin: JSON 参数（不含 tool 字段），如 {} 或 {"vx":0,"vy":0,"omega":300}

fn output_ok(msg: impl ToString) {
    println!("{}", json!({"output": msg.to_string(), "success": true}));
}

fn output_err(msg: impl ToString) {
    println!("{}", json!({"output": msg.to_string(), "success": false}));
    std::process::exit(1);
}

#[tokio::main]
async fn main() {
    let args_vec: Vec<String> = env::args().collect();
    let tool_name = args_vec.get(1).map(|s| s.as_str()).unwrap_or("unknown");

    let port = env::var("OMNI_CAR_PORT").unwrap_or_else(|_| "/dev/ttyACM0".into());
    let baud = env::var("OMNI_CAR_BAUD")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1_000_000u32);

    // 读取 stdin（参数 JSON）
    let mut stdin_buf = String::new();
    if io::stdin().read_to_string(&mut stdin_buf).is_err() {
        output_err("failed to read stdin");
        return;
    }
    let args: Value = serde_json::from_str(&stdin_buf).unwrap_or(Value::Object(Default::default()));

    // 打开串口
    let mut bus = match Bus::open(&port, baud) {
        Ok(b) => b,
        Err(e) => {
            output_err(format!("failed to open {port}: {e}"));
            return;
        }
    };

    match tool_name {
        "init_motors" => {
            match init_motors(&mut bus).await {
                Ok(_) => output_ok("Motors initialized in wheel mode"),
                Err(e) => output_err(format!("init_motors failed: {e}")),
            }
        }
        "move_base" => {
            let vx = args["vx"].as_f64().unwrap_or(0.0) as f32;
            let vy = args["vy"].as_f64().unwrap_or(0.0) as f32;
            let omega = args["omega"].as_f64().unwrap_or(0.0) as f32;
            let duration_secs = args["duration_secs"].as_f64();
            match apply_velocity(&mut bus, vx, vy, omega).await {
                Ok(_) => {
                    if let Some(secs) = duration_secs {
                        let secs = secs.clamp(0.0, 30.0);
                        tokio::time::sleep(Duration::from_secs_f64(secs)).await;
                        let _ = apply_velocity(&mut bus, 0.0, 0.0, 0.0).await;
                        output_ok(format!("move_base: vx={vx} vy={vy} omega={omega}, stopped after {secs}s"))
                    } else {
                        output_ok(format!("move_base: vx={vx} vy={vy} omega={omega}"))
                    }
                }
                Err(e) => output_err(format!("move_base failed: {e}")),
            }
        }
        "stop" => {
            match apply_velocity(&mut bus, 0.0, 0.0, 0.0).await {
                Ok(_) => output_ok("Stopped"),
                Err(e) => output_err(format!("stop failed: {e}")),
            }
        }
        "robot_estop" => {
            estop(&mut bus).await;
            output_ok("Emergency stop executed, torque disabled")
        }
        unknown => {
            output_err(format!("Unknown tool '{unknown}'. Expected: init_motors, move_base, stop, robot_estop"));
        }
    }
}
