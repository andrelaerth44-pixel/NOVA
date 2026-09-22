use std::{collections::{BTreeMap, BTreeSet}, fs, path::{Path, PathBuf}, process::Command};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dependency {
    pub version: Option<String>,
    pub path: Option<String>,
    pub git: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Manifest {
    pub name: String,
    pub version: String,
    pub entry: String,
    pub dependencies: BTreeMap<String, Dependency>,
    pub path: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedPackage {
    pub name: String,
    pub version: String,
    pub source: String,
    pub manifest: PathBuf,
}

fn unquote(value: &str) -> Result<String, String> {
    let value = value.trim();
    if value.len() < 2 || !value.starts_with('"') || !value.ends_with('"') {
        return Err(format!("expected quoted string, got {}", value));
    }
    let inner = &value[1..value.len() - 1];
    Ok(inner.to_string())
}

fn split_assignment(line: &str) -> Result<(&str, &str), String> {
    line.split_once('=').ok_or_else(|| format!("expected key = value, got {}", line))
}

fn parse_inline_dependency(value: &str) -> Result<Dependency, String> {
    let value = value.trim();
    if !value.starts_with('{') || !value.ends_with('}') {
        return Ok(Dependency {
            version: Some(unquote(value)?),
            path: None,
            git: None,
        });
    }

    let inner = &value[1..value.len() - 1];
    let mut version = None;
    let mut path = None;
    let mut git = None;
    for field in inner.split(',') {
        let field = field.trim();
        if field.is_empty() {
            continue;
        }
        let (key, value) = split_assignment(field)?;
        let key = key.trim();
        let value = unquote(value.trim())?;
        match key {
            "version" => version = Some(value),
            "path" => path = Some(value),
            "git" => git = Some(value),
            _ => return Err(format!("unknown dependency field {}", key)),
        }
    }
    if version.is_none() && path.is_none() && git.is_none() {
        return Err("dependency table must declare version, path or git".into());
    }
    Ok(Dependency { version, path, git })
}

pub fn parse_manifest(path: &Path) -> Result<Manifest, String> {
    let path = if path.is_dir() {
        path.join("nova.toml")
    } else {
        path.to_path_buf()
    };
    let path = fs::canonicalize(&path)
        .map_err(|e| format!("cannot read manifest {}: {}", path.display(), e))?;
    let text = fs::read_to_string(&path)
        .map_err(|e| format!("cannot read manifest {}: {}", path.display(), e))?;

    let mut section = String::new();
    let mut name = None;
    let mut version = None;
    let mut entry = None;
    let mut dependencies = BTreeMap::new();

    for (line_no, raw) in text.lines().enumerate() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            section = line[1..line.len() - 1].trim().to_string();
            if section != "package" && section != "dependencies" {
                return Err(format!("{}:{}: unsupported manifest section [{}]", path.display(), line_no + 1, section));
            }
            continue;
        }
        let (key, value) = split_assignment(line)
            .map_err(|e| format!("{}:{}: {}", path.display(), line_no + 1, e))?;
        let key = key.trim();
        let value = value.trim();
        match section.as_str() {
            "package" => match key {
                "name" => name = Some(unquote(value)?),
                "version" => version = Some(unquote(value)?),
                "entry" => entry = Some(unquote(value)?),
                "language" => { let _ = unquote(value)?; },
                _ => return Err(format!("{}:{}: unknown package field {}", path.display(), line_no + 1, key)),
            },
            "dependencies" => {
                dependencies.insert(key.to_string(), parse_inline_dependency(value)?);
            }
            _ => return Err(format!("{}:{}: fields must be inside [package] or [dependencies]", path.display(), line_no + 1)),
        }
    }

    let name = name.ok_or_else(|| format!("{}: missing [package] name", path.display()))?;
    let version = version.ok_or_else(|| format!("{}: missing [package] version", path.display()))?;
    validate_version(&version)?;
    let entry = entry.unwrap_or_else(|| "Main.nova".into());

    Ok(Manifest { name, version, entry, dependencies, path })
}

fn validate_version(version: &str) -> Result<(), String> {
    let core = version.strip_prefix('^').or_else(|| version.strip_prefix('~')).unwrap_or(version);
    let parts: Vec<&str> = core.split('.').collect();
    if parts.len() < 2 || parts.iter().any(|p| p.is_empty() || !p.chars().all(|c| c.is_ascii_digit())) {
        return Err(format!("invalid package version {}", version));
    }
    Ok(())
}

fn resolve_path(manifest: &Manifest, raw: &str) -> Result<PathBuf, String> {
    let base = manifest.path.parent().unwrap_or_else(|| Path::new("."));
    let candidate = base.join(raw);
    fs::canonicalize(&candidate)
        .map_err(|e| format!("dependency path {}: {}", candidate.display(), e))
}

fn visit(
    manifest: Manifest,
    stack: &mut Vec<PathBuf>,
    active: &mut BTreeSet<PathBuf>,
    seen: &mut BTreeSet<PathBuf>,
    out: &mut Vec<ResolvedPackage>,
    source: String,
) -> Result<(), String> {
    if active.contains(&manifest.path) {
        let mut cycle = stack.clone();
        cycle.push(manifest.path.clone());
        let names = cycle.iter().map(|p| p.display().to_string()).collect::<Vec<_>>();
        return Err(format!("package dependency cycle: {}", names.join(" -> ")));
    }
    if seen.contains(&manifest.path) {
        return Ok(());
    }

    active.insert(manifest.path.clone());
    stack.push(manifest.path.clone());

    for (alias, dependency) in &manifest.dependencies {
        if dependency.path.is_none() && dependency.git.is_none() {
            if dependency.version.is_none() {
                return Err(format!("dependency {} must specify version, path or git", alias));
            }
            return Err(format!(
                "dependency {} declares a registry version, but no NOVA registry is configured",
                alias
            ));
        }
        if let Some(raw) = &dependency.git {
            let cache = manifest.path.parent().unwrap_or_else(|| Path::new(".")).join(".nova").join("git").join(alias);
            if !cache.exists() {
                if let Some(parent) = cache.parent() { fs::create_dir_all(parent).map_err(|e| format!("cannot create git cache: {}",e))?; }
                let status = Command::new("git").args(["clone","--depth","1",raw,cache.to_string_lossy().as_ref()]).status()
                    .map_err(|e| format!("cannot invoke git for {}: {}",alias,e))?;
                if !status.success() { return Err(format!("git clone failed for {}",alias)); }
            }
            let dependency_manifest=parse_manifest(&cache)?;
            if dependency_manifest.name != *alias { return Err(format!("dependency alias {} does not match package name {}",alias,dependency_manifest.name)); }
            visit(dependency_manifest,stack,active,seen,out,format!("git:{}",raw))?;
        }
        if let Some(version) = &dependency.version {
            if dependency.path.is_none() && dependency.git.is_none() {
                let registry = std::env::var("NOVA_REGISTRY").map_err(|_| format!("dependency {} needs NOVA_REGISTRY or a path/git source",alias))?;
                let candidate=Path::new(&registry).join(alias).join(version.trim_start_matches(['^','~']));
                let dependency_manifest=parse_manifest(&candidate)?;
                if dependency_manifest.name != *alias { return Err(format!("registry package {} has manifest name {}",alias,dependency_manifest.name)); }
                visit(dependency_manifest,stack,active,seen,out,format!("registry:{}@{}",alias,version))?;
            }
        }
        if let Some(raw) = &dependency.path {
            let dependency_manifest = parse_manifest(&resolve_path(&manifest, raw)?)?;
            if dependency_manifest.name != *alias {
                return Err(format!(
                    "dependency alias {} does not match package name {}",
                    alias, dependency_manifest.name
                ));
            }
            visit(
                dependency_manifest,
                stack,
                active,
                seen,
                out,
                format!("path:{}", raw),
            )?;
        }
    }

    stack.pop();
    active.remove(&manifest.path);
    seen.insert(manifest.path.clone());

    out.push(ResolvedPackage {
        name: manifest.name,
        version: manifest.version,
        source,
        manifest: manifest.path,
    });
    Ok(())
}

pub fn resolve_manifest(path: &Path) -> Result<Vec<ResolvedPackage>, String> {
    let root = parse_manifest(path)?;
    let mut stack = Vec::new();
    let mut active = BTreeSet::new();
    let mut seen = BTreeSet::new();
    let mut packages = Vec::new();
    visit(root, &mut stack, &mut active, &mut seen, &mut packages, "root".into())?;
    packages.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(packages)
}

pub fn write_lock(path: &Path) -> Result<PathBuf, String> {
    let manifest = parse_manifest(path)?;
    let packages = resolve_manifest(&manifest.path)?;
    let lock_path = manifest.path.parent().unwrap_or_else(|| Path::new(".")).join("nova.lock");
    let root = manifest.path.parent().unwrap_or_else(|| Path::new("."));

    let mut text = String::from("format = 1\n\n");
    for package in packages {
        let source = if package.manifest == manifest.path {
            "root".to_string()
        } else {
            let rel = package.manifest.parent().unwrap_or_else(|| Path::new(".")).strip_prefix(root).unwrap_or(package.manifest.parent().unwrap_or_else(|| Path::new(".")));
            format!("path:{}", rel.display())
        };
        text.push_str("[[package]]\n");
        text.push_str(&format!("name = \"{}\"\n", package.name));
        text.push_str(&format!("version = \"{}\"\n", package.version));
        text.push_str(&format!("source = \"{}\"\n\n", source));
    }

    fs::write(&lock_path, text)
        .map_err(|e| format!("cannot write {}: {}", lock_path.display(), e))?;
    Ok(lock_path)
}
