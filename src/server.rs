use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, SystemTime};

/// Build once, then serve `out` over HTTP and rebuild whenever a source file
/// changes. Deliberately dependency-free: a polling watcher and a tiny
/// blocking file server are enough for a preview loop.
pub fn serve(root: &Path, out: &Path, port: u16) -> Result<(), String> {
    let base_path = base_path(root);
    report(crate::render::build(root, out));

    {
        let root = root.to_path_buf();
        let out = out.to_path_buf();
        std::thread::spawn(move || {
            let mut last = fingerprint(&root);
            loop {
                std::thread::sleep(Duration::from_millis(700));
                let now = fingerprint(&root);
                if now != last {
                    last = now;
                    println!("\nchange detected — rebuilding");
                    report(crate::render::build(&root, &out));
                }
            }
        });
    }

    let listener = TcpListener::bind(("127.0.0.1", port))
        .map_err(|e| format!("cannot bind 127.0.0.1:{port}: {e}"))?;
    println!("serving http://127.0.0.1:{port}{base_path}/  (ctrl-c to stop)");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let out = out.to_path_buf();
                let base_path = base_path.clone();
                std::thread::spawn(move || {
                    let _ = handle(stream, &out, &base_path);
                });
            }
            Err(e) => eprintln!("connection failed: {e}"),
        }
    }
    Ok(())
}

fn report(result: Result<usize, String>) {
    match result {
        Ok(n) => println!("built {n} pages"),
        Err(e) => eprintln!("build failed: {e}"),
    }
}

fn base_path(root: &Path) -> String {
    crate::config::Config::load(&root.join("site.toml"))
        .map(|c| c.base_path)
        .unwrap_or_default()
}

/// Modification times of everything the build reads. Any change to the set
/// (added, removed, or touched file) changes the fingerprint.
fn fingerprint(root: &Path) -> BTreeMap<PathBuf, SystemTime> {
    let mut map = BTreeMap::new();
    for dir in ["content", "static", "templates"] {
        for entry in walkdir::WalkDir::new(root.join(dir)).into_iter().flatten() {
            if let Ok(meta) = entry.metadata() {
                if let Ok(modified) = meta.modified() {
                    map.insert(entry.path().to_path_buf(), modified);
                }
            }
        }
    }
    if let Ok(meta) = std::fs::metadata(root.join("site.toml")) {
        if let Ok(modified) = meta.modified() {
            map.insert(root.join("site.toml"), modified);
        }
    }
    map
}

fn handle(mut stream: TcpStream, out: &Path, base_path: &str) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;

    // Drain headers so the client sees a clean response.
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 || line == "\r\n" || line == "\n" {
            break;
        }
    }

    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let target = parts.next().unwrap_or("/");
    if method != "GET" && method != "HEAD" {
        return respond(&mut stream, 405, "text/plain; charset=utf-8", b"method not allowed");
    }

    let path = target.split(['?', '#']).next().unwrap_or("/");
    let path = decode(path);
    let path = path.strip_prefix(base_path).unwrap_or(&path);

    match resolve(out, path) {
        Some(file) => {
            let mut body = Vec::new();
            std::fs::File::open(&file)?.read_to_end(&mut body)?;
            respond(&mut stream, 200, content_type(&file), &body)
        }
        None => {
            let custom = out.join("404.html");
            if custom.is_file() {
                let body = std::fs::read(custom)?;
                respond(&mut stream, 404, "text/html; charset=utf-8", &body)
            } else {
                respond(&mut stream, 404, "text/plain; charset=utf-8", b"not found")
            }
        }
    }
}

/// Maps a URL path to a file inside `out`, refusing anything that escapes it.
fn resolve(out: &Path, path: &str) -> Option<PathBuf> {
    let mut candidate = out.to_path_buf();
    for part in path.split('/').filter(|p| !p.is_empty()) {
        let component = Path::new(part);
        if component
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
        {
            return None;
        }
        candidate.push(part);
    }
    if candidate.is_dir() {
        candidate.push("index.html");
    }
    if candidate.is_file() && candidate.starts_with(out) {
        Some(candidate)
    } else {
        None
    }
}

fn respond(stream: &mut TcpStream, status: u16, mime: &str, body: &[u8]) -> std::io::Result<()> {
    let reason = match status {
        200 => "OK",
        404 => "Not Found",
        405 => "Method Not Allowed",
        _ => "Error",
    };
    let head = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()
}

fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "xml" => "application/xml; charset=utf-8",
        "txt" => "text/plain; charset=utf-8",
        "json" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "pdf" => "application/pdf",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        _ => "application/octet-stream",
    }
}

fn decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(byte);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traversal_is_refused() {
        let out = Path::new("/tmp/does-not-matter");
        assert!(resolve(out, "/../../etc/passwd").is_none());
        assert!(resolve(out, "/..%2f..%2fetc").is_none());
    }

    #[test]
    fn percent_escapes_are_decoded() {
        assert_eq!(decode("/a%20b/"), "/a b/");
    }
}
