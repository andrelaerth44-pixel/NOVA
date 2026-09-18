use crate::app_ast::*;

pub struct AppParser<'a> {
    lines: Vec<&'a str>,
    pos: usize,
}

impl<'a> AppParser<'a> {
    pub fn new(source: &'a str) -> Self {
        Self { lines: source.lines().collect(), pos: 0 }
    }

    fn next_nonempty(&mut self) -> Option<&'a str> {
        while self.pos < self.lines.len() {
            let line = self.lines[self.pos].trim();
            self.pos += 1;
            if !line.is_empty() && !line.starts_with("//") && !line.starts_with('#') {
                return Some(line);
            }
        }
        None
    }

    fn block(&mut self) -> Result<Vec<&'a str>, String> {
        let mut out = Vec::new();
        let mut depth = 1usize;
        while let Some(line) = self.next_nonempty() {
            if line == "{" { depth += 1; continue; }
            if line == "}" {
                depth -= 1;
                if depth == 0 { return Ok(out); }
                continue;
            }
            out.push(line);
        }
        Err("unterminated app block".into())
    }

    pub fn parse(&mut self) -> Result<AppDeclRoot, String> {
        let header = self.next_nonempty().ok_or("expected app declaration")?;
        let rest = header.strip_prefix("app ").ok_or("expected app "Name"")?;
        let name = rest.trim().trim_end_matches('{').trim().trim_matches('"').to_string();
        if name.is_empty() { return Err("app name cannot be empty".into()); }
        let mut root = AppDeclRoot::new(name);
        while let Some(line) = self.next_nonempty() {
            if line == "}" { break; }
            if line == "auth {" {
                let lines = self.block()?;
                let providers = lines.into_iter().filter_map(|x| x.strip_prefix("provider ").or_else(|| Some(x))).map(|x| x.trim().to_string()).collect();
                root.declarations.push(AppDecl::Auth(AuthDecl { providers }));
            } else if line.starts_with("database ") && line.ends_with('{') {
                let provider = line.strip_prefix("database ").unwrap().trim_end_matches('{').trim().to_string();
                let lines = self.block()?;
                let tables = lines.into_iter().filter_map(|x| x.strip_prefix("table ")).map(|x| x.trim().to_string()).collect();
                root.declarations.push(AppDecl::Database(DatabaseDecl { provider, tables }));
            } else if line == "storage {" {
                let lines = self.block()?;
                let buckets = lines.into_iter().filter_map(|x| x.strip_prefix("bucket ")).map(|x| x.trim().to_string()).collect();
                root.declarations.push(AppDecl::Storage(StorageDecl { buckets }));
            } else if line == "navigation {" {
                let lines = self.block()?;
                let routes = lines.into_iter().map(|x| RouteDecl { name: x.to_string() }).collect();
                root.declarations.push(AppDecl::Navigation(routes));
            } else if line.starts_with("screen ") && line.ends_with('{') {
                let name = line.strip_prefix("screen ").unwrap().trim_end_matches('{').trim().trim_matches('"').to_string();
                let lines = self.block()?;
                let mut items = Vec::new();
                for item in lines {
                    if let Some(r) = item.strip_prefix("list ") {
                        items.push(ScreenItem::List { resource: r.trim().to_string() });
                    } else if let Some(rest) = item.strip_prefix("button ") {
                        let mut parts = rest.splitn(2, " {");
                        let label = parts.next().unwrap_or("").trim().trim_matches('"').to_string();
                        let action = parts.next().unwrap_or("").trim_end_matches('}').trim().to_string();
                        items.push(ScreenItem::Button { label, action });
                    }
                }
                root.declarations.push(AppDecl::Screen(ScreenDecl { name, items }));
            } else {
                return Err(format!("unknown app declaration: {}", line));
            }
        }
        Ok(root)
    }
}
