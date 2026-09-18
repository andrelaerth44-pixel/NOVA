use crate::{lex, Parser, Stmt};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Default)]
pub struct ModuleLoader {
    cache: HashMap<PathBuf, Vec<Stmt>>,
    active: Vec<PathBuf>,
    active_set: HashSet<PathBuf>,
}

impl ModuleLoader {
    pub fn new() -> Self { Self::default() }

    fn canonical(path: &Path) -> Result<PathBuf, String> {
        fs::canonicalize(path)
            .map_err(|e| format!("cannot resolve module {}: {}", path.display(), e))
    }

    fn resolve(base_dir: &Path, import: &str) -> Result<PathBuf, String> {
        let raw = Path::new(import);
        let candidate = if raw.is_absolute() {
            raw.to_path_buf()
        } else {
            base_dir.join(raw)
        };
        Self::canonical(&candidate)
    }

    fn load_path(&mut self, path: PathBuf) -> Result<Vec<Stmt>, String> {
        let path = Self::canonical(&path)?;

        if self.active_set.contains(&path) {
            let start = self.active.iter().position(|p| p == &path).unwrap_or(0);
            let mut chain: Vec<String> = self.active[start..]
                .iter()
                .map(|p| p.display().to_string())
                .collect();
            chain.push(path.display().to_string());
            return Err(format!("module import cycle: {}", chain.join(" -> ")));
        }

        if let Some(program) = self.cache.get(&path) {
            return Ok(program.clone());
        }

        let src = fs::read_to_string(&path)
            .map_err(|e| format!("cannot read module {}: {}", path.display(), e))?;
        let tokens = lex(&src)
            .map_err(|e| format!("module {}: lexer error: {}", path.display(), e))?;
        let mut parser = Parser::new(tokens);
        let program = parser.program()
            .map_err(|e| format!("module {}: parse error: {}", path.display(), e))?;

        self.active_set.insert(path.clone());
        self.active.push(path.clone());

        let base = path.parent().unwrap_or_else(|| Path::new("."));
        let mut expanded = Vec::new();

        for stmt in program {
            match stmt {
                Stmt::Import(import) => {
                    let dependency = Self::resolve(base, &import)?;
                    expanded.extend(self.load_path(dependency)?);
                }
                other => expanded.push(other),
            }
        }

        self.active.pop();
        self.active_set.remove(&path);
        self.cache.insert(path, expanded.clone());
        Ok(expanded)
    }

    pub fn load_entry(&mut self, entry: &Path) -> Result<Vec<Stmt>, String> {
        self.load_path(entry.to_path_buf())
    }
}
