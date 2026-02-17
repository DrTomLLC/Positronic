use super::osc::OscEvent;

/// Semantic shell state derived from OSC markers (prompt vs running).
#[derive(Debug, Clone, Default)]
pub struct SemanticState {
    pub in_prompt: bool,
    pub in_command: bool,
    pub last_exit: Option<i32>,
    pub cwd: Option<String>,
}

impl SemanticState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn apply(&mut self, ev: &OscEvent) {
        match ev {
            OscEvent::Cwd(p) => self.cwd = Some(p.clone()),
            OscEvent::PromptStart => {
                self.in_prompt = true;
                self.in_command = false;
            }
            OscEvent::CommandStart => {
                self.in_command = true;
                self.in_prompt = false;
            }
            OscEvent::CommandExecuted => {
                self.in_command = true;
                self.in_prompt = false;
            }
            OscEvent::CommandFinished { exit_code } => {
                self.in_command = false;
                self.in_prompt = true;
                self.last_exit = *exit_code;
            }
            OscEvent::Unknown(_) => {}
        }
    }
}
