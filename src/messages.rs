use std::fmt;

#[derive(Debug)]
pub enum CommandMessage {
    ExecCommand {
        command_line: String,
        exec_id: Option<String>,
    },
}

#[derive(Debug, Deserialize, Clone)]
#[serde(tag = "type")]
pub enum EventMessage {
    ChannelAcceptedEvent {
        #[serde(rename = "channelName")]
        channel_name: String,
    },
    StringEvent {
        level: String,
        message: String,
        #[serde(rename = "channelName")]
        channel_name: Option<String>,
        #[serde(rename = "execId")]
        exec_id: Option<String>,
    },
    ExecStatusEvent {
        status: String,
        #[serde(rename = "channelName")]
        channel_name: Option<String>,
        #[serde(rename = "execId")]
        exec_id: Option<String>,
        #[serde(rename = "commandQueue")]
        command_queue: Option<Vec<String>>,
    },
}

impl fmt::Display for EventMessage {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &EventMessage::ChannelAcceptedEvent { ref channel_name } => {
                write!(f, "Bound to channel {}", channel_name)
            }
            &EventMessage::StringEvent {
                ref level,
                ref message,
                ..
            } => write!(f, "[{}] {}", level, message),
            &EventMessage::ExecStatusEvent { ref status, .. } => {
                write!(f, "[exec event] {}", status)
            }
        }
    }
}
