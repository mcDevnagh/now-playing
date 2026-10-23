use super::*;

const PLAY: &str = r#"{"jsonrpc":"2.0","method":"Player.OnPlay","params":{"data":{"item":{"id":7,"type":"movie"},"player":{"playerid":1,"speed":1}},"sender":"xbmc"}}"#;
const PAUSE: &str = r#"{"jsonrpc":"2.0","method":"Player.OnPause","params":{"data":{"item":{"id":7,"type":"movie"},"player":{"playerid":1,"speed":0}},"sender":"xbmc"}}"#;
const RESUME: &str = r#"{"jsonrpc":"2.0","method":"Player.OnResume","params":{"data":{"item":{"id":7,"type":"movie"},"player":{"playerid":1,"speed":1}},"sender":"xbmc"}}"#;
const STOP: &str = r#"{"jsonrpc":"2.0","method":"Player.OnStop","params":{"data":{"end":false,"item":{"id":7,"type":"movie"}},"sender":"xbmc"}}"#;
const VOLUME: &str = r#"{"jsonrpc":"2.0","method":"Application.OnVolumeChanged","params":{"data":{"muted":false,"volume":100},"sender":"xbmc"}}"#;

/// How many starts `input` gives, and how reading it ended.
fn starts(input: &str) -> (usize, io::Result<()>) {
    let mut starts = 0;
    let result = follow(input.as_bytes(), || starts += 1);
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
