use std::net::TcpListener;
use std::thread;

use super::*;

/// As seen on the HTPC, trimmed.
const PAGES: &str = r#"[ {
   "title": "QuickAccess_uid2",
   "webSocketDebuggerUrl": "ws://localhost:8080/devtools/page/074032A8D67AA916D9A359E6C19BDFE5"
}, {
   "title": "SharedJSContext",
   "webSocketDebuggerUrl": "ws://localhost:8080/devtools/page/0CFCD041A4FAD448687A06D13D35C2E8"
} ]"#;

#[test]
fn finds_the_shared_js_context() {
    assert_eq!(
        page_path(PAGES).unwrap(),
        "/devtools/page/0CFCD041A4FAD448687A06D13D35C2E8"
    );
}

#[test]
fn missing_page() {
    let err = page_path("[]").unwrap_err();
    assert!(err.contains("no SharedJSContext page"), "{err}");
}

#[test]
fn outcomes() {
    assert_eq!(outcome(&json!({"id": 1, "result": {"result": {}}})), Ok(()));
    let thrown = json!({"id": 1, "result": {"result": {}, "exceptionDetails": {
        "text": "Uncaught",
        "exception": {"description": "ReferenceError: SteamUIStore is not defined"},
    }}});
    assert_eq!(
        outcome(&thrown),
        Err("threw ReferenceError: SteamUIStore is not defined".to_owned())
    );
    let error = json!({"id": 1, "error": {"code": -32000, "message": "No such context"}});
    assert_eq!(
        outcome(&error),
        Err(r#"evaluate: "No such context""#.to_owned())
    );
}

/// A fake Steam debugger: serves the page list, then takes one websocket
/// connection, sends an event, and replies to the first message with
/// `reply`. Returns its address, and the expression it was sent.
fn fake_steam(status: &'static str, reply: Value) -> (String, thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap().to_string();
    let handle = thread::spawn(move || {
        let (mut http, _) = listener.accept().unwrap();
        // Read the whole request: closing with some unread resets the
        // connection.
        let mut request = Vec::new();
        while !request.ends_with(b"\r\n\r\n") {
            let mut byte = [0];
            http.read_exact(&mut byte).unwrap();
            request.push(byte[0]);
        }
        if request.starts_with(b"GET /json HTTP/1.0") {
            // As Chromium does.
            return String::new();
        }
        // Unchecked: given an error status, the client may close before
        // reading the body.
        let _ = write!(
            http,
            "HTTP/1.1 {status}\r\nContent-Length:{}\r\n\r\n{PAGES}",
            PAGES.len()
        );
        drop(http);
        if status != "200 OK" {
            return String::new();
        }

        let (stream, _) = listener.accept().unwrap();
        let mut socket = tungstenite::accept(stream).unwrap();
        let request: Value =
            serde_json::from_str(&socket.read().unwrap().into_text().unwrap()).unwrap();
        let event = json!({"method": "Runtime.consoleAPICalled", "params": {}});
        socket.send(Message::text(event.to_string())).unwrap();
        socket.send(Message::text(reply.to_string())).unwrap();
        assert_eq!(request["method"], "Runtime.evaluate");
        request["params"]["expression"].as_str().unwrap().to_owned()
    });
    (address, handle)
}

#[test]
fn evaluates_in_steams_ui() {
    let (address, steam) = fake_steam("200 OK", json!({"id": 1, "result": {"result": {}}}));
    assert_eq!(evaluate(&address, "SteamUIStore.SetRunningApp(7)"), Ok(()));
    assert_eq!(steam.join().unwrap(), "SteamUIStore.SetRunningApp(7)");
}

#[test]
fn reports_an_exception() {
    let reply = json!({"id": 1, "result": {"exceptionDetails": {"text": "Uncaught"}}});
    let (address, steam) = fake_steam("200 OK", reply);
    assert_eq!(evaluate(&address, "x"), Err("threw Uncaught".to_owned()));
    steam.join().unwrap();
}

/// Kodi's web server, say, on the port.
#[test]
fn not_steam() {
    let (address, steam) = fake_steam("404 Not Found", Value::Null);
    let err = evaluate(&address, "x").unwrap_err();
    assert!(
        err.contains("404 Not Found (not Steam's debugger?)"),
        "{err}"
    );
    steam.join().unwrap();
}

#[test]
fn nothing_listening() {
    let address = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .to_string();
    let err = evaluate(&address, "x").unwrap_err();
    assert!(err.contains(".cef-enable-remote-debugging"), "{err}");
}

/// Against a real Chromium, with a page standing in for Steam's UI:
///
/// ```sh
/// chromium --headless=new --remote-debugging-port=18080 \
///     "data:text/html,<title>SharedJSContext</title><script>
///     SteamUIStore={SetRunningApp(id){if(id!==7)throw Error(id)}}</script>"
/// CDP_ADDRESS=127.0.0.1:18080 cargo test -p now-playing-switcher -- --ignored
/// ```
#[test]
#[ignore = "needs a Chromium with remote debugging at $CDP_ADDRESS"]
fn real_chromium() {
    let address = std::env::var("CDP_ADDRESS").unwrap();
    assert_eq!(evaluate(&address, "SteamUIStore.SetRunningApp(7)"), Ok(()));
    let err = evaluate(&address, "SteamUIStore.SetRunningApp(8)").unwrap_err();
    assert!(err.contains("threw Error: 8"), "{err}");
}
