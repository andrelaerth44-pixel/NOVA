use crate::app_ast::*;

pub struct AppParser<'a> {
    source: &'a str,
}

impl<'a> AppParser<'a> {
    pub fn new(source: &'a str) -> Self {
        Self { source }
    }

    fn clean(line: &str) -> &str {
        let line = line.split("//").next().unwrap_or("").trim();
        line.strip_prefix('#').map(str::trim).unwrap_or(line)
    }

    fn parse_block(lines: &[String], index: &mut usize, kind: &str) -> Result<Vec<String>, String> {
        let mut out = Vec::new();
        let mut depth = 1usize;
        while *index < lines.len() {
            let line = Self::clean(&lines[*index]).to_string();
            *index += 1;
            if line.is_empty() {
                continue;
            }
            if line == "{" {
                depth += 1;
                continue;
            }
            if line == "}" {
                depth -= 1;
                if depth == 0 {
                    return Ok(out);
                }
                continue;
            }
            let opens = line.chars().filter(|c| *c == '{').count();
            let closes = line.chars().filter(|c| *c == '}').count();
            if opens > closes {
                depth += opens - closes;
            } else if closes > opens {
                let delta = closes - opens;
                if delta >= depth {
                    return Err(format!("unexpected closing brace in {} block", kind));
                }
                depth -= delta;
            }
            out.push(line);
        }
        Err(format!("unterminated {} block", kind))
    }

    fn split_inline_block(line: &str) -> Option<(&str, &str)> {
        let open = line.find('{')?;
        let close = line.rfind('}')?;
        if close <= open {
            return None;
        }
        Some((line[..open].trim(), line[open + 1..close].trim()))
    }

    fn quoted_name(text: &str, prefix: &str) -> Result<String, String> {
        let rest = text
            .strip_prefix(prefix)
            .ok_or_else(|| format!("expected {}", prefix))?
            .trim();
        let name = rest.trim_end_matches('{').trim();
        if name.len() < 2 || !name.starts_with('"') || !name.ends_with('"') {
            return Err(format!("expected quoted name after {}", prefix));
        }
        Ok(name[1..name.len() - 1].to_string())
    }

    pub fn parse(&mut self) -> Result<AppDeclRoot, String> {
        let lines: Vec<String> = self
            .source
            .lines()
            .map(Self::clean)
            .filter(|x| !x.is_empty())
            .map(ToOwned::to_owned)
            .collect();

        let mut index = 0usize;
        let header = lines.get(index).ok_or("expected app declaration")?;
        index += 1;
        let name = Self::quoted_name(header, "app ")?;
        let mut root = AppDeclRoot::new(name);

        while index < lines.len() {
            let line = &lines[index];
            if line == "}" {
                index += 1;
                break;
            }

            if line == "auth {" {
                index += 1;
                let block = Self::parse_block(&lines, &mut index, "auth")?;
                let mut providers = Vec::new();
                for item in block {
                    if let Some(provider) = item.strip_prefix("provider ") {
                        providers.push(provider.trim().to_string());
                    } else {
                        return Err(format!("unknown auth declaration: {}", item));
                    }
                }
                root.declarations.push(AppDecl::Auth(AuthDecl { providers }));
                continue;
            }

            if line.starts_with("database ") && line.ends_with('{') {
                let provider = line
                    .strip_prefix("database ")
                    .unwrap()
                    .trim_end_matches('{')
                    .trim()
                    .to_string();
                index += 1;
                let block = Self::parse_block(&lines, &mut index, "database")?;
                let mut tables = Vec::new();
                for item in block {
                    if let Some(table) = item.strip_prefix("table ") {
                        tables.push(table.trim().to_string());
                    } else {
                        return Err(format!("unknown database declaration: {}", item));
                    }
                }
                root.declarations.push(AppDecl::Database(DatabaseDecl { provider, tables }));
                continue;
            }

            if line == "storage {" {
                index += 1;
                let block = Self::parse_block(&lines, &mut index, "storage")?;
                let mut buckets = Vec::new();
                for item in block {
                    if let Some(bucket) = item.strip_prefix("bucket ") {
                        buckets.push(bucket.trim().to_string());
                    } else {
                        return Err(format!("unknown storage declaration: {}", item));
                    }
                }
                root.declarations.push(AppDecl::Storage(StorageDecl { buckets }));
                continue;
            }

            if line == "navigation {" {
                index += 1;
                let block = Self::parse_block(&lines, &mut index, "navigation")?;
                let mut routes = Vec::new();
                for item in block {
                    let name = item.trim();
                    if name.is_empty() || name.contains(' ') {
                        return Err(format!("invalid route: {}", item));
                    }
                    routes.push(RouteDecl { name: name.to_string() });
                }
                root.declarations.push(AppDecl::Navigation(routes));
                continue;
            }

            if line.starts_with("screen ") && line.ends_with('{') {
                let name = Self::quoted_name(line, "screen ")?;
                index += 1;
                let block = Self::parse_block(&lines, &mut index, "screen")?;
                let mut items = Vec::new();

                for item in block {
                    if let Some(resource) = item.strip_prefix("list ") {
                        items.push(ScreenItem::List {
                            resource: resource.trim().to_string(),
                        });
                        continue;
                    }

                    if let Some(rest) = item.strip_prefix("button ") {
                        if let Some((head, action)) = Self::split_inline_block(rest) {
                            let label = head.trim().trim_matches('"').to_string();
                            if label.is_empty() || action.is_empty() {
                                return Err(format!("invalid button declaration: {}", item));
                            }
                            items.push(ScreenItem::Button {
                                label,
                                action: action.to_string(),
                            });
                            continue;
                        }
                    }

                    return Err(format!("unknown screen declaration: {}", item));
                }

                root.declarations.push(AppDecl::Screen(ScreenDecl { name, items }));
                continue;
            }

            return Err(format!("unknown app declaration: {}", line));
        }

        if index < lines.len() {
            return Err("unexpected content after app block".into());
        }
        Ok(root)
    }
}
