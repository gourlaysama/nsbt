use std::fmt;

use languageserver_types::{LogMessageParams, MessageType};

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
#[serde()]
pub struct EventMessage {
    pub jsonrpc: String,
    pub method: String,
    pub params: LogMessageParams,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum Event {
    Log {
        #[serde(rename = "type")]
        typ: u8,
        message: String,
    },
}

impl fmt::Display for EventMessage {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let typ = match self.params.typ {
            MessageType::Error => "error",
            MessageType::Info => "info",
            MessageType::Log => "info",
            MessageType::Warning => "warn",
        };
        write!(f, "[{}] {}", typ, self.params.message)
    }
}
