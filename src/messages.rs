use std::fmt;

use languageserver_types::{DiagnosticSeverity, InitializeResult, LogMessageParams, MessageType,
                           PublishDiagnosticsParams};

#[derive(Debug, Serialize)]
#[serde(tag = "method", rename_all = "camelCase")]
pub enum CommandMessage {
    Initialize {
        id: u32,
        jsonrpc: String,
        params: SbtInitParams,
    },
    #[serde(rename = "sbt/exec")]
    SbtExec {
        id: u32,
        jsonrpc: String,
        params: SbtExecParams,
    },
    #[serde(rename = "sbt/setting")]
    SbtSettingQuery {
        id: u32,
        jsonrpc: String,
        params: SbtSettingQueryParams,
    },
}

impl CommandMessage {
    pub fn initialize(id: u32) -> CommandMessage {
        CommandMessage::Initialize {
            id: id,
            jsonrpc: "2.0".to_string(),
            params: SbtInitParams {
                initialization_options: None,
            },
        }
    }

    pub fn sbt_exec(id: u32, command_line: String) -> CommandMessage {
        CommandMessage::SbtExec {
            id: id,
            jsonrpc: "2.0".to_string(),
            params: SbtExecParams {
                command_line: command_line,
            },
        }
    }

    pub fn sbt_setting(id: u32, setting: String) -> CommandMessage {
        CommandMessage::SbtSettingQuery {
            id: id,
            jsonrpc: "2.0".to_string(),
            params: SbtSettingQueryParams { setting: setting },
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SbtInitParams {
    initialization_options: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SbtExecParams {
    command_line: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SbtSettingQueryParams {
    setting: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", untagged)]
pub enum RpcInput {
    Notification(EventMessage),
    CallResult(RpcAnswer),
    Error(RpcError),
}

#[derive(Debug, Deserialize)]
pub struct RpcError {
    pub id: String,
    pub error: RpcErrorResult,
}

#[derive(Debug, Deserialize)]
pub struct RpcErrorResult {
    code: i32,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct RpcAnswer {
    pub id: String,
    pub result: RpcAnswerResult,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum RpcAnswerResult {
    #[serde(rename_all = "camelCase")] Setting { value: String, content_type: String },
    Init(InitializeResult),
}

#[derive(Debug, Deserialize)]
#[serde(tag = "method", content = "params")]
pub enum EventMessage {
    #[serde(rename = "window/logMessage")] Log(LogMessageParams),
    #[serde(rename = "textDocument/publishDiagnostics")] Diagnostic(PublishDiagnosticsParams),
}

impl fmt::Display for EventMessage {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &EventMessage::Log(LogMessageParams { typ, ref message }) => {
                let typ = match typ {
                    MessageType::Error => "error",
                    MessageType::Info => "info",
                    MessageType::Log => "info",
                    MessageType::Warning => "warn",
                };
                write!(f, "[{}] {}", typ, message)
            }
            &EventMessage::Diagnostic(PublishDiagnosticsParams {
                ref uri,
                ref diagnostics,
            }) => {
                for d in diagnostics {
                    let typ = match d.severity {
                        Some(DiagnosticSeverity::Warning) => "warn",
                        Some(DiagnosticSeverity::Error) => "error",
                        _ => "info",
                    };
                    write!(
                        f,
                        "[{}] {}:{}:{}: {}\n",
                        typ,
                        uri.path(),
                        d.range.start.line + 1,
                        d.range.start.character + 1,
                        d.message
                    )?;
                }
                Ok(())
            }
        }
    }
}

impl fmt::Display for RpcError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "[error] {}", self.error.message)
    }
}

impl fmt::Display for RpcAnswer {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.result {
            RpcAnswerResult::Init(_) => Ok(()),
            RpcAnswerResult::Setting { ref value, .. } => write!(f, "[info] {}\n", value),
        }
    }
}

impl fmt::Display for RpcInput {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &RpcInput::Notification(ref ev) => ev.fmt(f),
            &RpcInput::CallResult(ref res) => res.fmt(f),
            &RpcInput::Error(ref e) => e.fmt(f),
        }
    }
}
