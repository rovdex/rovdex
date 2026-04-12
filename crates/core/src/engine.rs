use crate::{Context, Task};

#[derive(Debug, Default)]
pub struct Engine;

impl Engine {
    pub fn new() -> Self {
        Self
    }

    pub fn run(&self, _context: Context, task: Task) -> String {
        format!("Rovdex received task: {}", task.prompt)
    }
}
