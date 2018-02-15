use std::fmt;

use languageserver_types::{DiagnosticSeverity, LogMessageParams, MessageType,
                           PublishDiagnosticsParams};

#[derive(Debug, Serialize)]
#[serde(tag = "method", rename_all = "camelCase")]
pub enum CommandMessage {
    Initialize {
        id: u8,
        jsonrpc: String,
        params: SbtInitParams,
    },
    #[serde(rename = "sbt/exec")]
    SbtExec {
        id: u8,
        jsonrpc: String,
        params: SbtExecParams,
    },
}

impl CommandMessage {
    pub fn initialize(id: u8) -> CommandMessage {
        CommandMessage::Initialize {
            id: id,
            jsonrpc: "2.0".to_string(),
            params: SbtInitParams {
                initialization_options: None,
            },
        }
    }

    pub fn sbt_exec(id: u8, command_line: String) -> CommandMessage {
        CommandMessage::SbtExec {
            id: id,
            jsonrpc: "2.0".to_string(),
            params: SbtExecParams {
                command_line: command_line,
            },
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

#[derive(Debug, Deserialize)]
#[serde(tag = "method", content = "params")]
pub enum EventMessage {
    #[serde(rename = "window/logMessage")]
    Log(LogMessageParams),
    #[serde(rename = "textDocument/publishDiagnostics")]
    Diagnostic(PublishDiagnosticsParams),
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
                };
                Ok(())
            }
        }
    }
}
