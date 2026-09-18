#[derive(Clone, Debug, PartialEq)]
pub enum AppDecl {
    Auth(AuthDecl),
    Database(DatabaseDecl),
    Storage(StorageDecl),
    Navigation(Vec<RouteDecl>),
    Screen(ScreenDecl),
}

#[derive(Clone, Debug, PartialEq)]
pub struct AuthDecl { pub providers: Vec<String> }

#[derive(Clone, Debug, PartialEq)]
pub struct DatabaseDecl { pub provider: String, pub tables: Vec<String> }

#[derive(Clone, Debug, PartialEq)]
pub struct StorageDecl { pub buckets: Vec<String> }

#[derive(Clone, Debug, PartialEq)]
pub struct RouteDecl { pub name: String }

#[derive(Clone, Debug, PartialEq)]
pub struct ScreenDecl {
    pub name: String,
    pub items: Vec<ScreenItem>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ScreenItem {
    List { resource: String },
    Button { label: String, action: String },
}

#[derive(Clone, Debug, PartialEq)]
pub struct AppDeclRoot {
    pub name: String,
    pub declarations: Vec<AppDecl>,
}

impl AppDeclRoot {
    pub fn new(name: String) -> Self {
        Self { name, declarations: Vec::new() }
    }
}
