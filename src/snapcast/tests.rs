use super::*;
use crate::fixtures::meta_status;

fn stream_id<'a>(server: &'a Server, client: &str) -> Option<&'a str> {
    server.stream_for(client).map(|s| s.id.as_str())
}

#[test]
fn client_matching() {
    let mut server = meta_status();
    assert_eq!(stream_id(&server, "HTPC"), Some("Spotify-HTPC"));
    assert_eq!(stream_id(&server, "htpc"), Some("Spotify-HTPC"));
    assert_eq!(stream_id(&server, "Kitchen"), None);
    // A connected client in a group whose stream doesn't exist.
    assert_eq!(server.groups[0].clients[0].host.name, "phone");
    server.groups[0].clients[0].connected = true;
    assert_eq!(stream_id(&server, "phone"), None);
}

#[test]
fn host_name_matches_when_id_does_not() {
    let mut server = meta_status();
    server.groups[1].clients[0].id = "living-room".into();
    assert_eq!(stream_id(&server, "htpc"), Some("Spotify-HTPC"));
}

#[test]
fn disconnected_client_has_no_stream() {
    let mut server = meta_status();
    server.groups[1].clients[0].connected = false;
    assert_eq!(stream_id(&server, "HTPC"), None);
}

#[test]
fn follows_group_stream() {
    let mut server = meta_status();
    let mut other = server.streams[0].clone();
    other.id = "Meta-HTPC".into();
    server.streams.push(other);
    server.groups[1].stream_id = "Meta-HTPC".into();
    assert_eq!(stream_id(&server, "HTPC"), Some("Meta-HTPC"));
}
