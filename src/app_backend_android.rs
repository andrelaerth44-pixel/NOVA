use crate::app_ast::*;

pub fn validate_android_target(app: &AppDeclRoot) -> Result<(), String> {
    if app.name.trim().is_empty() { return Err("Android app name cannot be empty".into()); }
    for decl in &app.declarations {
        match decl {
            AppDecl::Auth(a) if a.providers.is_empty() => return Err("auth block requires a provider".into()),
            AppDecl::Database(d) if d.provider.is_empty() => return Err("database provider cannot be empty".into()),
            AppDecl::Storage(s) if s.buckets.is_empty() => return Err("storage block requires at least one bucket".into()),
            _ => {}
        }
    }
    Ok(())
}

pub fn emit_android_project(app: &AppDeclRoot) -> Result<String, String> {
    validate_android_target(app)?;
    let package = format!("com.nova.generated.{}", sanitize(&app.name));
    let routes = app.declarations.iter().find_map(|d| {
        if let AppDecl::Navigation(r) = d { Some(r.clone()) } else { None }
    }).unwrap_or_default();
    let mut route_code = String::new();
    for route in &routes {
        route_code.push_str(&format!("    composable(\"{}\") {{ Text(\"{}\") }}\n", route.name, route.name));
    }
    let source = format!(r#"package {package}

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController

class MainActivity : ComponentActivity() {{
    override fun onCreate(savedInstanceState: Bundle?) {{
        super.onCreate(savedInstanceState)
        setContent {{ NovaApp() }}
    }}
}}

@Composable
fun NovaApp() {{
    val nav = rememberNavController()
    MaterialTheme {{
        NavHost(navController = nav, startDestination = "{start}") {{
{routes}
        }}
    }}
}}
"#, package=package, start=routes.first().map(|r| r.name.clone()).unwrap_or_else(|| "home".into()), routes=route_code);
    Ok(source)
}

fn sanitize(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() { out.push(c.to_ascii_lowercase()); } else if out.ends_with('_') { continue } else { out.push('_'); }
    }
    if out.is_empty() { "app".into() } else { out }
}
