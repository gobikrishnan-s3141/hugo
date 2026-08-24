mod config;
mod content;
mod feed;
mod markdown;
mod render;
mod server;

use chrono::Local;
use std::path::{Path, PathBuf};
use std::process::Command;

const USAGE: &str = "\
quill — a tiny static site generator

usage:
  quill build [--out DIR]     render content/ into public/ (default DIR: public)
  quill serve [PORT]          build, serve locally, and rebuild on change (default 1313)
  quill new SECTION TITLE     scaffold content/SECTION/<slug>/index.md
  quill publish [MESSAGE]     build, then commit and push (CI deploys the result)
";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    let result = match args.first().map(String::as_str) {
        Some("build") => cmd_build(&root, &args[1..]),
        Some("serve") => cmd_serve(&root, &args[1..]),
        Some("new") => cmd_new(&root, &args[1..]),
        Some("publish") => cmd_publish(&root, &args[1..]),
        Some("-h") | Some("--help") | Some("help") | None => {
            print!("{USAGE}");
            return;
        }
        Some(other) => Err(format!("unknown command `{other}`\n\n{USAGE}")),
    };

    if let Err(message) = result {
        eprintln!("error: {message}");
        std::process::exit(1);
    }
}

fn out_dir(root: &Path, args: &[String]) -> PathBuf {
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        if arg == "--out" {
            if let Some(dir) = it.next() {
                return root.join(dir);
            }
        }
    }
    root.join("public")
}

fn cmd_build(root: &Path, args: &[String]) -> Result<(), String> {
    let out = out_dir(root, args);
    let started = std::time::Instant::now();
    let pages = render::build(root, &out)?;
    println!(
        "built {pages} pages into {} in {:?}",
        out.display(),
        started.elapsed()
    );
    Ok(())
}

fn cmd_serve(root: &Path, args: &[String]) -> Result<(), String> {
    let port = args
        .iter()
        .find(|a| !a.starts_with("--"))
        .map(|a| a.parse::<u16>().map_err(|_| format!("bad port `{a}`")))
        .transpose()?
        .unwrap_or(1313);
    server::serve(root, &out_dir(root, args), port)
}

fn cmd_new(root: &Path, args: &[String]) -> Result<(), String> {
    let [section, rest @ ..] = args else {
        return Err(format!("`new` needs a section and a title\n\n{USAGE}"));
    };
    if rest.is_empty() {
        return Err(format!("`new` needs a title\n\n{USAGE}"));
    }
    let title = rest.join(" ");
    let slug = render::slugify(&title);
    let today = Local::now().format("%Y-%m-%d").to_string();

    let section_dir = root.join("content").join(section);
    let index = section_dir.join("_index.md");
    if !index.exists() {
        std::fs::create_dir_all(&section_dir).map_err(|e| format!("{}: {e}", section_dir.display()))?;
        let title_case = capitalise(section);
        std::fs::write(
            &index,
            format!("---\ntitle: \"{title_case}\"\ndescription: \"\"\n---\n"),
        )
        .map_err(|e| format!("{}: {e}", index.display()))?;
        println!("created {}", index.display());
    }

    let dir = section_dir.join(&slug);
    let file = dir.join("index.md");
    if file.exists() {
        return Err(format!("{} already exists", file.display()));
    }
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let body = format!(
        "---\n\
         title: \"{title}\"\n\
         date: {today}\n\
         tags: []\n\
         summary: \"\"\n\
         ---\n\n\
         Write here. Drop figures and PDFs in this folder and link them by name,\n\
         for example `![](figure.png)` or `[the paper](paper.pdf)`.\n"
    );
    std::fs::write(&file, body).map_err(|e| format!("{}: {e}", file.display()))?;
    println!("created {}", file.display());
    Ok(())
}

fn cmd_publish(root: &Path, args: &[String]) -> Result<(), String> {
    let message = if args.is_empty() {
        format!("Update site — {}", Local::now().format("%Y-%m-%d %H:%M"))
    } else {
        args.join(" ")
    };

    // Never push something that does not build.
    render::build(root, &root.join("public"))?;

    git(root, &["add", "-A"])?;
    let staged = Command::new("git")
        .args(["diff", "--cached", "--quiet"])
        .current_dir(root)
        .status()
        .map_err(|e| format!("running git: {e}"))?;
    if staged.success() {
        println!("nothing to publish — working tree is clean");
        return Ok(());
    }
    git(root, &["commit", "-m", &message])?;
    git(root, &["push"])?;
    println!("pushed — the deploy workflow will publish the site");
    Ok(())
}

fn git(root: &Path, args: &[&str]) -> Result<(), String> {
    let status = Command::new("git")
        .args(args)
        .current_dir(root)
        .status()
        .map_err(|e| format!("running git {}: {e}", args.join(" ")))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("git {} failed", args.join(" ")))
    }
}

fn capitalise(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}
