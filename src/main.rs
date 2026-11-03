use pulldown_cmark::{html, CowStr, Event, Options, Parser, Tag};
use std::env;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Component, Path, PathBuf};

const STYLE: &str = r#"
:root{color-scheme:light dark;--bg:#f6f5f2;--surface:#fff;--text:#242522;--muted:#74766f;--line:#e7e5df;--accent:#2b6658;--code:#eeede8;--link:#256b59}
@media(prefers-color-scheme:dark){:root{--bg:#171916;--surface:#20231f;--text:#e9e9e3;--muted:#a0a398;--line:#343831;--accent:#a8d4b9;--code:#292c27;--link:#a8d4b9}}
*{box-sizing:border-box}body{margin:0;background:var(--bg);color:var(--text);font:16px/1.7 Inter,ui-sans-serif,system-ui,-apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif;-webkit-font-smoothing:antialiased}
a{color:var(--link);text-decoration-thickness:1px;text-underline-offset:3px}a:hover{text-decoration-thickness:2px}.shell{max-width:920px;margin:0 auto;padding:48px 28px 88px}.top{display:flex;justify-content:space-between;align-items:center;border-bottom:1px solid var(--line);padding-bottom:20px;margin-bottom:46px}.brand{font-size:14px;font-weight:650;letter-spacing:.02em;color:var(--text);text-decoration:none}.hint{color:var(--muted);font-size:13px}.eyebrow{font-size:12px;letter-spacing:.12em;text-transform:uppercase;color:var(--accent);font-weight:700;margin:0 0 10px}h1{font-size:clamp(2rem,5vw,3.25rem);line-height:1.12;letter-spacing:-.045em;margin:0 0 14px;font-weight:720} .intro{color:var(--muted);margin:0 0 38px}.files{list-style:none;padding:0;margin:0;border-top:1px solid var(--line)}.files li{border-bottom:1px solid var(--line)}.files a{display:flex;align-items:baseline;justify-content:space-between;gap:20px;padding:16px 2px;color:var(--text);text-decoration:none}.files a:hover .name{color:var(--link)}.name{font-weight:570}.path{font-size:13px;color:var(--muted);text-align:right;overflow-wrap:anywhere}.empty{border:1px dashed var(--line);padding:24px;border-radius:8px;color:var(--muted)}article{max-width:720px;margin:0 auto}article h1{margin-bottom:34px}article h2,article h3,article h4{line-height:1.3;letter-spacing:-.025em;margin:2em 0 .65em}article h2{font-size:1.65rem}article h3{font-size:1.3rem}article p,article ul,article ol,article blockquote{margin:0 0 1.25em}article li{padding-left:.2em}article blockquote{margin-left:0;padding:.1em 0 .1em 1.15em;border-left:3px solid var(--accent);color:var(--muted)}code{font: .88em ui-monospace,SFMono-Regular,Menlo,monospace;background:var(--code);padding:.15em .35em;border-radius:4px}pre{overflow:auto;background:var(--code);padding:18px 20px;border-radius:8px;line-height:1.55}pre code{background:none;padding:0}hr{border:0;border-top:1px solid var(--line);margin:2em 0}img{max-width:100%;height:auto}table{border-collapse:collapse;display:block;overflow:auto;margin:1.5em 0}th,td{border:1px solid var(--line);padding:8px 12px;text-align:left}th{background:var(--code)}.back{display:inline-block;margin-bottom:30px;font-size:14px}.not-found{color:var(--muted)}@media(max-width:600px){.shell{padding:28px 20px 60px}.top{margin-bottom:34px}.files a{align-items:flex-start;flex-direction:column;gap:2px}.path{text-align:left}}
"#;

fn escape_html(value: &str) -> String {
    value.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
        .replace('"', "&quot;").replace('\'', "&#39;")
}

fn page(title: &str, content: &str) -> String {
    format!("<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><meta name=\"color-scheme\" content=\"light dark\"><title>{} · mdserve</title><style>{STYLE}</style></head><body><div class=\"shell\"><header class=\"top\"><a class=\"brand\" href=\"/\">mdserve</a><span class=\"hint\">Local Markdown library</span></header>{}</div></body></html>", escape_html(title), content)
}

fn markdown_files(root: &Path, dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(|entry| entry.file_name().to_string_lossy().to_lowercase());
    for entry in entries {
        let path = entry.path();
        let Ok(kind) = entry.file_type() else { continue };
        if kind.is_dir() {
            if !entry.file_name().to_string_lossy().starts_with('.') { markdown_files(root, &path, files); }
        } else if kind.is_file() && path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("md") || ext.eq_ignore_ascii_case("markdown")) {
            if let Ok(relative) = path.strip_prefix(root) { files.push(relative.to_path_buf()); }
        }
    }
}

fn url_path(path: &Path) -> String {
    path.components().filter_map(|part| match part { Component::Normal(s) => Some(s.to_string_lossy().into_owned()), _ => None })
        .map(|segment| segment.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect::<String>())
        .collect::<Vec<_>>().join("/")
}

fn index(root: &Path) -> String {
    let mut files = Vec::new();
    markdown_files(root, root, &mut files);
    let list = if files.is_empty() {
        "<p class=\"empty\">No Markdown files yet. Add a <code>.md</code> file to this folder and refresh.</p>".to_string()
    } else {
        let items = files.iter().map(|path| {
            let display = path.with_extension("").to_string_lossy().replace(['/', '\\'], " / ");
            format!("<li><a href=\"/{}\"><span class=\"name\">{}</span><span class=\"path\">{}</span></a></li>", url_path(path), escape_html(path.file_stem().unwrap_or_default().to_string_lossy().as_ref()), escape_html(&display))
        }).collect::<String>();
        format!("<ul class=\"files\">{items}</ul>")
    };
    page("Your documents", &format!("<main><p class=\"eyebrow\">Your library</p><h1>Markdown, in one place.</h1><p class=\"intro\">{} document{} in this folder</p>{}</main>", files.len(), if files.len() == 1 { "" } else { "s" }, list))
}

fn safe_destination(dest: &str) -> bool {
    let lower = dest.trim().to_ascii_lowercase();
    !(lower.starts_with("javascript:") || lower.starts_with("data:") || lower.starts_with("vbscript:"))
}

fn render_markdown(source: &str) -> String {
    let parser = Parser::new_ext(source, Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS)
        .map(|event| match event {
            Event::Html(_) | Event::InlineHtml(_) => Event::Text(CowStr::from("")),
            Event::Start(Tag::Link { link_type, dest_url, title, id }) if !safe_destination(&dest_url) =>
                Event::Start(Tag::Link { link_type, dest_url: CowStr::from("#"), title, id }),
            Event::Start(Tag::Image { link_type, dest_url, title, id }) if !safe_destination(&dest_url) =>
                Event::Start(Tag::Image { link_type, dest_url: CowStr::from("#"), title, id }),
            other => other,
        });
    let mut output = String::new();
    html::push_html(&mut output, parser);
    output
}

fn response(stream: &mut TcpStream, status: &str, body: &str) -> std::io::Result<()> {
    write!(stream, "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\nX-Content-Type-Options: nosniff\r\nContent-Security-Policy: default-src 'none'; style-src 'unsafe-inline'; img-src https: http: data:; base-uri 'none'; form-action 'none'\r\n\r\n{body}", body.len())
}

fn handle(mut stream: TcpStream, root: &Path) -> std::io::Result<()> {
    let mut buffer = [0; 8192];
    let count = stream.read(&mut buffer)?;
    let request = String::from_utf8_lossy(&buffer[..count]);
    let Some(target) = request.lines().next().and_then(|line| line.split_whitespace().nth(1)) else { return Ok(()) };
    if target == "/" {
        return response(&mut stream, "200 OK", &index(root));
    }
    let decoded = percent_decode(target.trim_start_matches('/'));
    let relative = Path::new(&decoded);
    if relative.components().any(|component| !matches!(component, Component::Normal(_))) {
        return response(&mut stream, "400 Bad Request", &page("Bad request", "<main class=\"not-found\"><h1>Invalid path</h1><a class=\"back\" href=\"/\">← Back to library</a></main>"));
    }
    let candidate = root.join(relative);
    let Ok(path) = candidate.canonicalize() else {
        return response(&mut stream, "404 Not Found", &page("Not found", "<main class=\"not-found\"><h1>Document not found</h1><a class=\"back\" href=\"/\">← Back to library</a></main>"));
    };
    if !path.starts_with(root) || !path.is_file() || !path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("md") || ext.eq_ignore_ascii_case("markdown")) {
        return response(&mut stream, "404 Not Found", &page("Not found", "<main class=\"not-found\"><h1>Document not found</h1><a class=\"back\" href=\"/\">← Back to library</a></main>"));
    }
    let source = fs::read_to_string(&path).unwrap_or_default();
    let title = path.file_stem().unwrap_or_default().to_string_lossy();
    let content = format!("<main><a class=\"back\" href=\"/\">← All documents</a><article><h1>{}</h1>{}</article></main>", escape_html(&title), render_markdown(&source));
    response(&mut stream, "200 OK", &page(&title, &content))
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(hex) = u8::from_str_radix(&value[i + 1..i + 3], 16) { decoded.push(hex); i += 3; continue; }
        }
        decoded.push(bytes[i]); i += 1;
    }
    String::from_utf8_lossy(&decoded).into_owned()
}

fn main() -> std::io::Result<()> {
    let mut args = env::args().skip(1);
    let root = args.next().map(PathBuf::from).unwrap_or(env::current_dir()?);
    let address = args.next().unwrap_or_else(|| "127.0.0.1:8345".to_string());
    let root = root.canonicalize()?;
    if !root.is_dir() { return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "the Markdown root must be a directory")); }
    let listener = TcpListener::bind(&address)?;
    println!("Serving Markdown from {} at http://{}", root.display(), address);
    for stream in listener.incoming().flatten() { let _ = handle(stream, &root); }
    Ok(())
}
