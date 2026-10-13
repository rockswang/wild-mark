//! 三个独立原型的共享设施：Win32 封装 + 自采样 + 消息泵。
//! 见 plan.md 第 4 节；本 crate 只做技术验证，不做产品化封装。

pub mod harness;
pub mod md2rtf;
pub mod metrics;
pub mod richedit;
pub mod scintilla;
pub mod win;
