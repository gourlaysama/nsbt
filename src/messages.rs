use std::fmt;

#[derive(Debug)]
pub enum CommandMessage {
    ExecCommand {
        command_line: String,
        exec_id: Option<String>,
    },
}

#[derive(Debug)]
pub enum EventMessage {
    ChannelAcceptedEvent { channel_name: String },
    LogEvent {
        level: String,
        message: String,
        channel_name: Option<String>,
        exec_id: Option<String>,
    },
    ExecStatusEvent {
        status: String,
        channel_name: Option<String>,
        exec_id: Option<String>,
        command_queue: Vec<String>,
    },
}

impl fmt::Display for EventMessage {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &EventMessage::ChannelAcceptedEvent { ref channel_name } => {
                write!(f, "Bound to channel {}", channel_name)
            }
            &EventMessage::LogEvent { ref level, ref message, .. } => {
                write!(f, "[{}] {}", level, message)
            }
            &EventMessage::ExecStatusEvent { ref status, .. } => {
                write!(f, "[exec event] {}", status)
            }
        }
    }
}
