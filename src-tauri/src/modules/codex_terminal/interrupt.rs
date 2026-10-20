use super::*;

impl CodexTerminalService {
    pub fn interrupt_turn(&self, task_id: &str, conversation_id: &str) -> AppResult<()> {
        self.control_execution(task_id, conversation_id, "interrupt")
    }
}
