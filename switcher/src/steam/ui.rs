//! Steam's UI, through its CEF remote debugging port. Steam opens the port
//! when it starts if `~/.steam/steam/.cef-enable-remote-debugging` exists.

use std::fmt::Display;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use serde::Deserialize;
use serde_json::{Value, json};
use tungstenite::Message;

/// Steam listens on IPv4 only. Not `localhost`: that tries `::1` first,
/// where Kodi's web server also uses port 8080.
const DEBUGGER: &str = "127.0.0.1:8080";
const TIMEOUT: Duration = Duration::from_secs(2);
/// The page Steam's UI code runs in.
const PAGE: &str = "SharedJSContext";

/// Brings `app` to the front as Steam's own menu does: by moving it to the
/// front of Steam's running apps, from which Steam tells gamescope what to
/// show. Writing gamescope's property directly doesn't last, as Steam
/// rewrites it from that list whenever its overlay changes.
pub fn focus(app: u32) -> Result<(), String> {
    evaluate(DEBUGGER, &format!("SteamUIStore.SetRunningApp({app})"))
        .map_err(|e| format!("Steam's UI at {DEBUGGER}: {e}"))
}

/// Launches the app with `launch_id`, as `steam steam://rungameid/<id>`
/// would, but through the running Steam: a `steam` process started by a
/// service might not find it. Steam brings the app to the front once it's
/// up. Launching an app that's already running makes Steam show an error.
pub fn launch(launch_id: u64) -> Result<(), String> {
    let url = format!("steam://rungameid/{launch_id}");
    evaluate(
        DEBUGGER,
        &format!("SteamClient.URL.ExecuteSteamURL({url:?})"),
    )
    .map_err(|e| format!("Steam's UI at {DEBUGGER}: {e}"))
}

/// Shows Steam's library home in front of the app in front, which keeps
/// running behind it.
pub fn home() -> Result<(), String> {
    evaluate(DEBUGGER, HOME).map_err(|e| format!("Steam's UI at {DEBUGGER}: {e}"))
}

const HOME: &str = r#"SteamUIStore.Navigate("/library/home")"#;

/// Whether Steam's UI is up: its page is open, and has set up the store the
/// other calls use.
pub fn is_up() -> Result<(), String> {
    evaluate(DEBUGGER, "SteamUIStore").map_err(|e| format!("Steam's UI at {DEBUGGER}: {e}"))
}

/// Evaluates `expression` in Steam's UI, failing if it throws.
fn evaluate(address: &str, expression: &str) -> Result<(), String> {
    let path = page_path(&get(address, "/json")?)?;
    let url = format!("ws://{address}{path}");
    let (mut socket, _) = tungstenite::client(url, connect(address)?).map_err(err("handshake"))?;
    let request = json!({
        "id": 1,
        "method": "Runtime.evaluate",
        "params": { "expression": expression },
    });
    socket
        .send(Message::text(request.to_string()))
        .map_err(err("send"))?;
    loop {
        let Message::Text(text) = socket.read().map_err(err("read"))? else {
            continue;
        };
        let reply: Value = serde_json::from_str(&text).map_err(err("reply"))?;
        // Anything else is an event, not the reply.
        if reply["id"] == 1 {
            return outcome(&reply);
        }
    }
}

fn connect(address: &str) -> Result<TcpStream, String> {
    let stream = TcpStream::connect(address).map_err(|e| {
        format!(
            "{e} (is ~/.steam/steam/.cef-enable-remote-debugging there, and Steam restarted since?)"
        )
    })?;
    stream
        .set_read_timeout(Some(TIMEOUT))
        .map_err(err("socket"))?;
    stream
        .set_write_timeout(Some(TIMEOUT))
        .map_err(err("socket"))?;
    Ok(stream)
}

/// The body of an HTTP GET of `path`. HTTP/1.1, as Chromium's debugger
/// closes HTTP/1.0 requests without a reply.
fn get(address: &str, path: &str) -> Result<String, String> {
    let mut stream = connect(address)?;
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n"
    )
    .map_err(err(path))?;
    let mut reader = BufReader::new(stream);
    let mut status = String::new();
    reader.read_line(&mut status).map_err(err(path))?;
    let status = status.trim_end();
    if status.split(' ').nth(1) != Some("200") {
        let status = if status.is_empty() {
            "no reply"
        } else {
            status
        };
        return Err(format!("{path}: {status} (not Steam's debugger?)"));
    }
    let mut length = None;
    loop {
        let mut header = String::new();
        reader.read_line(&mut header).map_err(err(path))?;
        let header = header.trim_end();
        if header.is_empty() {
            break;
        }
        // Chromium writes "Content-Length:1946", without a space.
        if let Some((name, value)) = header.split_once(':')
            && name.eq_ignore_ascii_case("content-length")
        {
            length = value.trim().parse::<usize>().ok();
        }
    }
    let mut body = Vec::new();
    match length {
        Some(length) => {
            body.resize(length, 0);
            reader.read_exact(&mut body).map_err(err(path))?;
        }
        None => {
            reader.read_to_end(&mut body).map_err(err(path))?;
        }
    }
    String::from_utf8(body).map_err(err(path))
}

#[derive(Deserialize)]
struct Page {
    title: String,
    #[serde(rename = "webSocketDebuggerUrl")]
    url: String,
}

/// The path of the websocket for Steam's UI page, from the page list.
/// Steam's URLs name `localhost`, so only the path is used.
fn page_path(pages: &str) -> Result<String, String> {
    let pages: Vec<Page> = serde_json::from_str(pages).map_err(err("/json"))?;
    let page = pages
        .into_iter()
        .find(|page| page.title == PAGE)
        .ok_or_else(|| format!("no {PAGE} page (Steam's UI isn't up yet?)"))?;
    let path = page
        .url
        .strip_prefix("ws://")
        .and_then(|rest| rest.find('/').map(|slash| &rest[slash..]))
        .ok_or_else(|| format!("{PAGE}: unexpected URL {}", page.url))?;
    Ok(path.to_owned())
}

/// Whether a `Runtime.evaluate` reply says the expression ran.
fn outcome(reply: &Value) -> Result<(), String> {
    if let Some(error) = reply.get("error") {
        return Err(format!("evaluate: {}", error["message"]));
    }
    match reply["result"].get("exceptionDetails") {
        Some(exception) => {
            let description = &exception["exception"]["description"];
            let text = description.as_str().or(exception["text"].as_str());
            Err(format!("threw {}", text.unwrap_or("an exception")))
        }
        None => Ok(()),
    }
}

/// Prefixes an error with what it was about.
fn err<E: Display>(what: &str) -> impl Fn(E) -> String {
    move |e| format!("{what}: {e}")
}

#[cfg(test)]
mod tests;
