#[derive(Debug, PartialEq)]
pub enum CommandType {
    Native(String, Vec<String>),
    Legacy(String),
    Sandboxed(String),
    Neural(String),
    Script(String, String), // Type (run/wasm), Path
    Hive(HiveCommand),
    IO(IOCommand),
}

#[derive(Debug, PartialEq)]
pub enum HiveCommand {
    Scan,
    Status,
    Chat(String),
}

#[derive(Debug, PartialEq)]
pub enum IOCommand {
    Scan,
    Connect(String, u32), // Port, Baud
    List,
}

pub struct CommandParser;

impl CommandParser {
    pub fn parse(input: &str) -> CommandType {
        let trimmed = input.trim();

        if trimmed.starts_with("!") {
            let parts: Vec<&str> = trimmed[1..].split_whitespace().collect();
            if let Some(cmd) = parts.first() {
                if *cmd == "ai" || *cmd == "ask" {
                    // Extract the prompt (everything after the command)
                    let prompt = trimmed[1..].trim_start_matches(cmd).trim().to_string();
                    return CommandType::Neural(prompt);
                }

                if *cmd == "run" {
                    if let Some(path) = parts.get(1) {
                        return CommandType::Script("run".to_string(), path.to_string());
                    }
                }

                if *cmd == "wasm" {
                    if let Some(path) = parts.get(1) {
                        return CommandType::Script("wasm".to_string(), path.to_string());
                    }
                }

                if *cmd == "hive" {
                    if let Some(subcmd) = parts.get(1) {
                        match *subcmd {
                            "scan" => return CommandType::Hive(HiveCommand::Scan),
                            "status" => return CommandType::Hive(HiveCommand::Status),
                            _ => {}
                        }
                    }
                }

                if *cmd == "chat" {
                    let msg = trimmed[1..].trim_start_matches(cmd).trim().to_string();
                    return CommandType::Hive(HiveCommand::Chat(msg));
                }

                if *cmd == "io" {
                    if let Some(subcmd) = parts.get(1) {
                        match *subcmd {
                            "scan" | "list" => return CommandType::IO(IOCommand::Scan),
                            "connect" => {
                                if let (Some(port), Some(baud_str)) = (parts.get(2), parts.get(3)) {
                                    if let Ok(baud) = baud_str.parse::<u32>() {
                                        return CommandType::IO(IOCommand::Connect(
                                            port.to_string(),
                                            baud,
                                        ));
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }

                let args = parts.iter().skip(1).map(|s| s.to_string()).collect();
                return CommandType::Native(cmd.to_string(), args);
            }
        }

        if trimmed.starts_with("sandbox ") {
            let cmd = trimmed.trim_start_matches("sandbox ").to_string();
            return CommandType::Sandboxed(cmd);
        }

        CommandType::Legacy(input.to_string())
    }
}
