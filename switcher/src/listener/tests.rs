use super::*;

/// How many listeners `config` gets, and which players `Others` reaches,
/// as (kodi, snapcast).
fn listeners(config: &str) -> (usize, (bool, bool)) {
    let (listeners, others) = from_config(&Config::parse(config).unwrap()).unwrap();
    (listeners.len(), others.reaches())
}

const TABLES: &str = r#"
    [kodi]
    launch = 10375237194288726016
    launch_at_start = true

    [snapcast]
    app = 2
    server = "s:1705"
    client = "htpc"

    [uxplay]
    app = 3
"#;

#[test]
fn without_quiet_others_only_switching_tables_listen() {
    assert_eq!(listeners(TABLES), (2, (false, false)));
}

#[test]
fn with_quiet_others_every_table_listens() {
    let config = format!("quiet_others = true\n{TABLES}");
    assert_eq!(listeners(&config), (3, (true, true)));
}

#[test]
fn quiet_others_reaches_only_the_tables_given() {
    let config = "quiet_others = true\n[uxplay]\napp = 3\n[kodi]\napp = 1";
    assert_eq!(listeners(config), (2, (true, false)));
}
