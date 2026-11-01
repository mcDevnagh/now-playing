use std::time::Instant;

use super::*;

fn command(args: &[&str]) -> Vec<String> {
    args.iter().map(|&arg| arg.to_owned()).collect()
}

#[test]
fn quitting_on_its_own_is_reported() {
    let (tx, rx) = mpsc::channel();
    let _child = spawn(&command(&["sh", "-c", "exit 3"]), 7, tx).unwrap();
    let report = rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(
        report,
        Request::ScreensaverExited {
            id: 7,
            status: "exit status: 3".to_owned(),
        }
    );
}

#[test]
fn dropping_it_kills_it_unreported() {
    let (tx, rx) = mpsc::channel();
    let child = spawn(&command(&["sleep", "30"]), 1, tx).unwrap();
    let start = Instant::now();
    drop(child);
    // The waiting thread ends, dropping its sender, without a report.
    assert_eq!(
        rx.recv_timeout(Duration::from_secs(5)),
        Err(RecvTimeoutError::Disconnected)
    );
    assert!(start.elapsed() < Duration::from_secs(5));
}

#[test]
fn a_missing_program_fails_to_start() {
    let (tx, _rx) = mpsc::channel();
    assert!(spawn(&command(&["/nonexistent/now-playing"]), 1, tx).is_err());
}
