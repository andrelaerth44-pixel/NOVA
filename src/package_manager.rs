//! Package-manager operations layered on the deterministic manifest resolver.
use std::{fs,path::{Path,PathBuf},process::Command};
use crate::package::{parse_manifest,write_lock};

fn copy_dir(src:&Path,dst:&Path)->Result<(),String>{
    fs::create_dir_all(dst).map_err(|e|e.to_string())?;
    for entry in fs::read_dir(src).map_err(|e|e.to_string())?{
        let entry=entry.map_err(|e|e.to_string())?;let p=entry.path();let target=dst.join(entry.file_name());
        if p.is_dir(){copy_dir(&p,&target)?}else{fs::copy(&p,&target).map_err(|e|e.to_string())?;}
    }
    Ok(())
}
pub fn init(dir:&Path,name:&str)->Result<PathBuf,String>{
    fs::create_dir_all(dir).map_err(|e|e.to_string())?;
    let manifest=dir.join("nova.toml");
    if manifest.exists(){return Err(format!("{} already exists",manifest.display()));}
    let text=format!("[package]\nname = \"{}\"\nversion = \"0.1.0\"\n\n[dependencies]\n",name);
    fs::write(&manifest,text).map_err(|e|e.to_string())?;Ok(manifest)
}
pub fn install(path:&Path)->Result<PathBuf,String>{
    let manifest=parse_manifest(path)?;let root=manifest.path.parent().unwrap_or_else(||Path::new("."));
    let vendor=root.join(".nova").join("packages");fs::create_dir_all(&vendor).map_err(|e|e.to_string())?;
    for (alias,dep) in &manifest.dependencies{
        let destination=vendor.join(alias);
        if destination.exists(){continue;}
        if let Some(raw)=&dep.path{
            let source=root.join(raw).canonicalize().map_err(|e|e.to_string())?;
            copy_dir(&source,&destination)?;
        }else if let Some(raw)=&dep.git{
            let status=Command::new("git").args(["clone","--depth","1",raw,destination.to_string_lossy().as_ref()]).status().map_err(|e|e.to_string())?;
            if !status.success(){return Err(format!("git install failed for {}",alias));}
        }else if let Some(version)=&dep.version{
            let registry=std::env::var("NOVA_REGISTRY").map_err(|_|format!("{} requires NOVA_REGISTRY",alias))?;
            let source=Path::new(&registry).join(alias).join(version.trim_start_matches(['^','~']));
            copy_dir(&source,&destination)?;
        }
    }
    write_lock(&manifest.path)
}


pub fn init_app(dir:&Path,name:&str)->Result<PathBuf,String>{
    fs::create_dir_all(dir).map_err(|e|e.to_string())?;
    let manifest=dir.join("nova.toml");
    if manifest.exists(){return Err(format!("{} already exists",manifest.display()));}
    let pkg=sanitize_name(name);
    let files:[(&str,&str);9]=[
        ("Main.nova", r#"import "Activity.nova"

fn main() {
    Activity.start("MainLayout")
}
"#),
        ("Activity.nova", r#"import "MainLayout.nova"

fn start(layout) {
    print "NOVA Activity.start -> " + layout
    MainLayout.render()
}
"#),
        ("MainLayout.nova", r#"import "EditText.nova"
import "NotesAdapter.nova"

fn render() {
    title = "NOVA Notes"
    editor = EditText.create("Write a note...")
    adapter = NotesAdapter.create()
    print title
    print editor
    print adapter
}
"#),
        ("EditText.nova", r#"fn create(hint) {
    return "EditText(hint=" + hint + ")"
}
"#),
        ("NotesAdapter.nova", r#"import "Repository.nova"

fn create() {
    notes = Repository.all()
    return "NotesAdapter(count=" + len(notes) + ")"
}
"#),
        ("Repository.nova", r#"import "DAO.nova"

fn all() {
    return DAO.find_all()
}

fn save(note) {
    return DAO.insert(note)
}
"#),
        ("DAO.nova", r#"import "Note.nova"

fn find_all() {
    return [Note.create("Welcome to NOVA")]
}

fn insert(note) {
    print "DAO.insert"
    return note
}
"#),
        ("Note.nova", r#"struct Note {
    title: string
}

fn create(title) {
    return Note { title: title }
}
"#),
        ("README.nova", r#"# NOVA application source
# Every application source module is .nova.
# Entry point: Main.nova
"#),
    ];
    for (path,text) in files {
        fs::write(dir.join(path),text).map_err(|e|format!("cannot write {}: {}",path,e))?;
    }
    let manifest_text=format!("[package]\nname = \\\"{}\\\"\nversion = \\\"0.1.0\\\"\nlanguage = \\\"nova\\\"\nentry = \\\"Main.nova\\\"\n\n[dependencies]\n",pkg);
    fs::write(&manifest,manifest_text).map_err(|e|e.to_string())?;
    Ok(manifest)
}

fn sanitize_name(name:&str)->String{
    let mut out=String::new();
    for ch in name.chars(){ if ch.is_ascii_alphanumeric()||ch=='-'||ch=='_' {out.push(ch.to_ascii_lowercase())}else{out.push('-')} }
    if out.is_empty(){"nova-app".into()}else{out}
}
