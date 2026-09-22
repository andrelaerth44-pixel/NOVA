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


pub fn emit_android_project_files(app:&AppDeclRoot)->Result<Vec<(String,String)>,String>{
    validate_android_target(app)?;
    let package=format!("com.nova.generated.{}",sanitize(&app.name));
    let activity=emit_android_project(app)?;
    let manifest=format!(r#"<manifest xmlns:android="http://schemas.android.com/apk/res/android">
    <application android:theme="@style/Theme.Material3.DayNight.NoActionBar" android:label="{}">
        <activity android:name=".MainActivity" android:exported="true">
            <intent-filter>
                <action android:name="android.intent.action.MAIN"/>
                <category android:name="android.intent.category.LAUNCHER"/>
            </intent-filter>
        </activity>
    </application>
</manifest>
"#,app.name);
    let settings=r#"pluginManagement { repositories { google(); mavenCentral(); gradlePluginPortal() } }
dependencyResolutionManagement { repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS); repositories { google(); mavenCentral() } }
rootProject.name = "NOVA-Android"
include(":app")
"#.to_string();
    let root=r#"plugins {
    id("com.android.application") version "8.7.2" apply false
    id("org.jetbrains.kotlin.android") version "2.0.21" apply false
}
"#.to_string();
    let gradle_properties="org.gradle.jvmargs=-Xmx2048m -Dfile.encoding=UTF-8\nandroid.useAndroidX=true\nkotlin.code.style=official\n".to_string();
    let app_gradle=format!(r#"plugins {{
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}}
android {{
    namespace = "{package}"
    compileSdk = 35
    defaultConfig {{
        applicationId = "{package}"
        minSdk = 26
        targetSdk = 35
        versionCode = 1
        versionName = "1.0"
    }}
}}
dependencies {{
    implementation("androidx.core:core-ktx:1.15.0")
    implementation("androidx.activity:activity-compose:1.10.0")
    implementation("androidx.compose.ui:ui:1.7.6")
    implementation("androidx.compose.material3:material3:1.3.1")
    implementation("androidx.navigation:navigation-compose:2.8.5")
}}
"#);
    let strings=r#"<resources><string name="app_name">NOVA</string></resources>"#.to_string();
    let values_dir=format!("app/src/main/res/values/strings.xml");
    Ok(vec![
        ("settings.gradle.kts".into(),settings),
        ("build.gradle.kts".into(),root),
        ("gradle.properties".into(),gradle_properties),
        ("app/build.gradle.kts".into(),app_gradle),
        ("app/src/main/AndroidManifest.xml".into(),manifest),
        (format!("app/src/main/java/{}/MainActivity.kt",package.replace('.','/')),activity),
        (values_dir,strings),
    ])
}
