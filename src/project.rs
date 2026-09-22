use crate::{compiler, module_loader::ModuleLoader, package};
use std::{fs, path::{Path, PathBuf}};

#[derive(Debug)]
pub struct ProjectBuild {
    pub root: PathBuf,
    pub entry: PathBuf,
    pub source_files: Vec<PathBuf>,
    pub program: Vec<crate::Stmt>,
    pub compilation: compiler::Compilation,
}

fn collect_nova(root: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = fs::read_dir(root).map_err(|e| format!("cannot scan {}: {}", root.display(), e))?;
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.file_name().and_then(|x| x.to_str()) == Some(".nova") { continue; }
        if path.is_dir() {
            if path.file_name().and_then(|x| x.to_str()) == Some("target") { continue; }
            collect_nova(&path, out)?;
        } else if path.extension().and_then(|x| x.to_str()) == Some("nova") {
            out.push(path);
        }
    }
    Ok(())
}

pub fn load(dir: &Path) -> Result<ProjectBuild, String> {
    let manifest = package::parse_manifest(dir)?;
    let root = manifest.path.parent().unwrap_or_else(|| Path::new(".")).to_path_buf();
    let entry = root.join(&manifest.entry);
    if !entry.is_file() { return Err(format!("project entry {} does not exist", entry.display())); }

    let mut files = Vec::new();
    collect_nova(&root, &mut files)?;
    files.sort();
    files.dedup();

    // Parse every application source independently first. This guarantees that
    // a project cannot hide syntax errors in an unimported .nova file.
    for file in &files {
        let source = fs::read_to_string(file).map_err(|e| format!("{}: {}", file.display(), e))?;
        compiler::parse_source(&source).map_err(|e| format!("{}: {}", file.display(), e))?;
    }

    let mut loader = ModuleLoader::new();
    let program = loader.load_entry(&entry)?;
    let compilation = compiler::compile_program(program.clone())?;
    Ok(ProjectBuild { root, entry, source_files: files, program, compilation })
}

pub fn check(dir: &Path) -> Result<ProjectBuild, String> { load(dir) }

pub fn report(build: &ProjectBuild) -> String {
    let mut out = String::new();
    out.push_str(&format!("NOVA project: {}\n", build.root.display()));
    out.push_str(&format!("entry: {}\n", build.entry.display()));
    out.push_str(&format!("sources: {} .nova files\n", build.source_files.len()));
    for file in &build.source_files {
        let rel = file.strip_prefix(&build.root).unwrap_or(file);
        out.push_str(&format!("  {}\n", rel.display()));
    }
    out.push_str(&format!("ssa functions: {}\n", build.compilation.ssa_functions.len()));
    out.push_str("application source language: NOVA\n");
    out
}

pub fn source_set_is_nova_only(dir: &Path) -> Result<(), String> {
    fn walk(root: &Path) -> Result<(), String> {
        for entry in fs::read_dir(root).map_err(|e| format!("cannot scan {}: {}", root.display(), e))? {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            if path.file_name().and_then(|x| x.to_str()) == Some(".nova") || path.file_name().and_then(|x| x.to_str()) == Some("target") { continue; }
            if path.is_dir() { walk(&path)?; continue; }
            let ext = path.extension().and_then(|x| x.to_str()).unwrap_or("");
            if matches!(ext, "kt"|"java"|"c"|"cc"|"cpp"|"h"|"hpp"|"gradle") || path.file_name().and_then(|x| x.to_str()) == Some("build.gradle.kts") {
                return Err(format!("foreign application source is not allowed: {}", path.display()));
            }
        }
        Ok(())
    }
    walk(dir)
}
