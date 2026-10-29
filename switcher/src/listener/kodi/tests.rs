use serde_json::json;

use super::*;

const PLAY: &str = r#"{"jsonrpc":"2.0","method":"Player.OnPlay","params":{"data":{"item":{"id":7,"type":"movie"},"player":{"playerid":1,"speed":1}},"sender":"xbmc"}}"#;
const PAUSE: &str = r#"{"jsonrpc":"2.0","method":"Player.OnPause","params":{"data":{"item":{"id":7,"type":"movie"},"player":{"playerid":1,"speed":0}},"sender":"xbmc"}}"#;
const RESUME: &str = r#"{"jsonrpc":"2.0","method":"Player.OnResume","params":{"data":{"item":{"id":7,"type":"movie"},"player":{"playerid":1,"speed":1}},"sender":"xbmc"}}"#;
const STOP: &str = r#"{"jsonrpc":"2.0","method":"Player.OnStop","params":{"data":{"end":false,"item":{"id":7,"type":"movie"}},"sender":"xbmc"}}"#;
const VOLUME: &str = r#"{"jsonrpc":"2.0","method":"Application.OnVolumeChanged","params":{"data":{"muted":false,"volume":100},"sender":"xbmc"}}"#;

/// How many starts `input` gives, read as connection 1's messages, and how
/// reading it ended.
fn starts(input: &str) -> (usize, io::Result<()>) {
    let mut follow = Follow::new(1, Vec::new());
    let mut starts = 0;
    let result = read_messages(input.as_bytes(), |message| {
        let input = Input::Message {
            connection: 1,
            message,
        };
        follow.input(input, &mut || starts += 1).unwrap();
        true
    });
    (starts, result)
}

#[test]
fn play_and_resume_start() {
    let input = [PLAY, PAUSE, RESUME, STOP].concat();
    let (count, result) = starts(&input);
    assert_eq!(count, 2);
    result.unwrap();
}

#[test]
fn everything_else_is_ignored() {
    let input = [
        VOLUME,
        PAUSE,
        STOP,
        r#"{"id":1,"jsonrpc":"2.0","result":"OK"}"#,
    ]
    .concat();
    let (count, result) = starts(&input);
    assert_eq!(count, 0);
    result.unwrap();
}

#[test]
fn messages_split_by_whitespace_too() {
    let input = format!("{PLAY}\n{VOLUME}\r\n  {RESUME}\n");
    let (count, result) = starts(&input);
    assert_eq!(count, 2);
    result.unwrap();
}

#[test]
fn nothing_is_fine() {
    let (count, result) = starts("");
    assert_eq!(count, 0);
    result.unwrap();
}

/// Messages before the bad one still count; then the connection is
/// dropped, since the stream can't be resynced.
#[test]
fn malformed_ends_the_connection() {
    let (count, result) = starts(&format!("{PLAY}{{\"method\": oops}}{RESUME}"));
    assert_eq!(count, 1);
    assert!(result.is_err());
}

#[test]
fn cut_off_mid_message_is_an_error() {
    let (count, result) = starts(&PLAY[..PLAY.len() - 5]);
    assert_eq!(count, 0);
    assert!(result.is_err());
}

fn message(connection: u64, message: Value) -> Input {
    Input::Message {
        connection,
        message,
    }
}

/// Connection 1's reply to request `id`, with `result`.
fn reply(id: u64, result: Value) -> Input {
    message(1, json!({"id": id, "jsonrpc": "2.0", "result": result}))
}

/// A `Player.GetActivePlayers` result with these player ids, as Kodi
/// sends it.
fn players(ids: &[u64]) -> Value {
    let players = ids
        .iter()
        .map(|id| json!({"playerid": id, "playertype": "internal", "type": "video"}));
    Value::Array(players.collect())
}

/// Feeds `inputs` to connection 1, returning the lines written. Checks that
/// every `Player.PlayPause` written sets `"play": false`, so no test can
/// write a toggle.
fn written(inputs: Vec<Input>) -> Vec<Value> {
    let mut follow = Follow::new(1, Vec::new());
    for input in inputs {
        follow.input(input, &mut || {}).unwrap();
    }
    let out = String::from_utf8(follow.into_out()).unwrap();
    let lines: Vec<Value> = out
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    for line in &lines {
        if line["method"] == "Player.PlayPause" {
            assert_eq!(line["params"]["play"], false, "{line}");
        }
    }
    lines
}

fn get_active_players(id: u64) -> Value {
    json!({"id": id, "jsonrpc": "2.0", "method": "Player.GetActivePlayers"})
}

fn play_pause(id: u64, player: u64) -> Value {
    json!({
        "id": id,
        "jsonrpc": "2.0",
        "method": "Player.PlayPause",
        "params": {"playerid": player, "play": false},
    })
}

#[test]
fn pause_request_line() {
    assert_eq!(
        pause_request(7, 1),
        r#"{"id":7,"jsonrpc":"2.0","method":"Player.PlayPause","params":{"play":false,"playerid":1}}"#
    );
}

#[test]
fn pause_asks_for_active_players() {
    assert_eq!(written(vec![Input::Pause]), [get_active_players(1)]);
}

#[test]
fn active_players_are_paused() {
    let got = written(vec![Input::Pause, reply(1, players(&[1]))]);
    assert_eq!(got, [get_active_players(1), play_pause(2, 1)]);

    let got = written(vec![Input::Pause, reply(1, players(&[0, 1]))]);
    assert_eq!(
        got,
        [get_active_players(1), play_pause(2, 0), play_pause(3, 1)]
    );
}

#[test]
fn nothing_active_writes_nothing() {
    let got = written(vec![Input::Pause, reply(1, players(&[]))]);
    assert_eq!(got, [get_active_players(1)]);
}

#[test]
fn error_reply_writes_nothing() {
    let error = json!({
        "id": 1,
        "jsonrpc": "2.0",
        "error": {"code": -32100, "message": "Failed to execute method."},
    });
    let got = written(vec![Input::Pause, message(1, error)]);
    assert_eq!(got, [get_active_players(1)]);
}

#[test]
fn only_our_get_active_players_replies_pause() {
    let got = written(vec![
        // A reply to nothing we sent.
        reply(5, players(&[1])),
        Input::Pause,
        reply(1, players(&[1])),
        // The same reply again, and the `PlayPause` reply.
        reply(1, players(&[1])),
        reply(2, json!("OK")),
    ]);
    assert_eq!(got, [get_active_players(1), play_pause(2, 1)]);
}

#[test]
fn pauses_and_starts_together() {
    let mut follow = Follow::new(1, Vec::new());
    let mut starts = 0;
    let play: Value = serde_json::from_str(PLAY).unwrap();
    for input in [Input::Pause, message(1, play), reply(1, players(&[1]))] {
        follow.input(input, &mut || starts += 1).unwrap();
    }
    assert_eq!(starts, 1);
    assert_eq!(
        String::from_utf8(follow.into_out())
            .unwrap()
            .lines()
            .count(),
        2
    );
}

#[test]
fn old_connections_are_ignored() {
    let play: Value = serde_json::from_str(PLAY).unwrap();
    let mut follow = Follow::new(2, Vec::new());
    let mut starts = 0;
    let error = io::Error::new(io::ErrorKind::UnexpectedEof, "old");
    for input in [
        message(1, play),
        Input::Pause,
        // Connection 1's reply with the id connection 2 just used.
        reply(1, players(&[1])),
        Input::Closed {
            connection: 1,
            error,
        },
    ] {
        follow.input(input, &mut || starts += 1).unwrap();
    }
    assert_eq!(starts, 0);
    let out = String::from_utf8(follow.into_out()).unwrap();
    assert_eq!(out.lines().count(), 1, "{out}");

    let mut follow = Follow::new(2, Vec::new());
    let error = io::Error::new(io::ErrorKind::UnexpectedEof, "ours");
    let close = Input::Closed {
        connection: 2,
        error,
    };
    assert!(follow.input(close, &mut || {}).is_err());
}

#[test]
fn pauses_from_before_a_connection_are_discarded() {
    let (controller, inbox) = channel();
    controller.pause();
    controller.pause();
    inbox.discard_queued();
    assert!(inbox.rx.try_recv().is_err());
    // Later ones still arrive.
    controller.pause();
    assert!(matches!(inbox.rx.try_recv(), Ok(Input::Pause)));
}

/// `follow` over a real socket, with a fake Kodi: a pause from before the
/// connection is dropped, a later one pauses, and Kodi closing ends it.
#[test]
fn follow_over_a_socket() {
    use std::io::{BufRead, Write};
    use std::net::TcpListener;
    use std::time::Duration;

    let kodi = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = kodi.local_addr().unwrap();
    let (controller, inbox) = channel();
    controller.pause();
    let (started, starts) = mpsc::channel();
    let connection = thread::spawn(move || {
        let stream = TcpStream::connect(address).unwrap();
        follow(stream, 1, &inbox, &mut || started.send(()).unwrap())
    });

    let (mut socket, _) = kodi.accept().unwrap();
    socket
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut lines = BufReader::new(socket.try_clone().unwrap()).lines();
    let mut next_line =
        || -> Value { serde_json::from_str(&lines.next().unwrap().unwrap()).unwrap() };

    // Once a start is reported, the connection is past its discard.
    socket.write_all(PLAY.as_bytes()).unwrap();
    starts.recv_timeout(Duration::from_secs(5)).unwrap();
    controller.pause();
    assert_eq!(next_line(), get_active_players(1));
    let players = json!({"id": 1, "jsonrpc": "2.0", "result": players(&[1])});
    socket.write_all(players.to_string().as_bytes()).unwrap();
    assert_eq!(next_line(), play_pause(2, 1));

    drop(lines);
    drop(socket);
    let error = connection.join().unwrap().unwrap_err();
    assert_eq!(error.to_string(), "Kodi closed the connection");
}
