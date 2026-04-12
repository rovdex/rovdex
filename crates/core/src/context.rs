#[derive(Debug, Clone, Default)]
pub struct Context {
    pub cwd: String,
    pub repository_root: Option<String>,
}
