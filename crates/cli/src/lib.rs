//! gb-cli 的库部分（零依赖）：判定逻辑、gates 运行与报告、PNG 编码、最小 JSON。
//! 二进制入口在 `main.rs`；单测在 `crates/cli/tests/`。

pub mod baseline;
pub mod emu;
pub mod gates;
pub mod json;
pub mod judge;
pub mod png;
pub mod replay;
pub mod report;
