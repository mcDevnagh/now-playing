use super::*;

#[test]
fn backoff_doubles_up_to_the_cap() {
    let mut backoff = Backoff::new();
    let secs: Vec<_> = (0..7).map(|_| backoff.next_delay().as_secs()).collect();
    assert_eq!(secs, [1, 2, 4, 8, 16, 30, 30]);
}

#[test]
fn backoff_resets() {
    let mut backoff = Backoff::new();
    backoff.next_delay();
    backoff.next_delay();
    backoff.reset();
    assert_eq!(backoff.next_delay(), MIN_BACKOFF);
}
