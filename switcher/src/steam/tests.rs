use super::*;

const NOW_PLAYING: u32 = 2457742405;

/// Joins arguments as `/proc/<pid>/cmdline` does.
fn cmdline(args: &[&str]) -> Vec<u8> {
    args.iter()
        .flat_map(|arg| [arg.as_bytes(), b"\0"].concat())
        .collect()
}

/// As seen on the HTPC.
fn reaper(app_id: &str, command: &str) -> Vec<u8> {
    cmdline(&[
        "/var/home/me/.local/share/Steam/ubuntu12_32/reaper",
        "SteamLaunch",
        app_id,
        "--",
        command,
    ])
}

#[test]
fn reaper_launching_the_app() {
    let cmdline = reaper("AppId=2457742405", "/home/me/.local/bin/now-playing");
    assert!(launches(&cmdline, NOW_PLAYING));
}

#[test]
fn reaper_launching_another_app() {
    for app_id in ["AppId=2415673154", "AppId=24577424050", "AppId=245774240"] {
        assert!(!launches(&reaper(app_id, "kodi"), NOW_PLAYING), "{app_id}");
    }
}

#[test]
fn other_processes() {
    let cases = [
        // The app's own arguments, after `--`.
        cmdline(&["reaper", "SteamLaunch", "--", "app", "AppId=2457742405"]),
        cmdline(&["/usr/bin/notreaper", "SteamLaunch", "AppId=2457742405"]),
        cmdline(&["reaper", "AppId=2457742405"]),
        cmdline(&["grep", "AppId=2457742405"]),
        Vec::new(),
    ];
    for cmdline in cases {
        assert!(!launches(&cmdline, NOW_PLAYING), "{cmdline:?}");
    }
}

#[test]
fn scans_pid_directories() {
    let proc = std::env::temp_dir().join(format!("switcher-proc-{}", std::process::id()));
    let _ = fs::remove_dir_all(&proc);
    let add = |name: &str, cmdline: &[u8]| {
        fs::create_dir_all(proc.join(name)).unwrap();
        fs::write(proc.join(name).join("cmdline"), cmdline).unwrap();
    };
    add("1", &cmdline(&["/sbin/init"]));
    // Not a process, though it looks like one.
    add("self", &reaper("AppId=2415673154", "kodi"));
    // Gone mid-scan: no cmdline.
    fs::create_dir_all(proc.join("2")).unwrap();
    assert!(!is_running_in(&proc, 2415673154).unwrap());

    add("4652", &reaper("AppId=2457742405", "now-playing"));
    assert!(is_running_in(&proc, NOW_PLAYING).unwrap());
    fs::remove_dir_all(&proc).unwrap();
}

#[test]
fn missing_proc_is_an_error() {
    assert!(is_running_in(Path::new("/nonexistent/proc"), NOW_PLAYING).is_err());
    let err = display_of_in(Path::new("/nonexistent/proc"), NOW_PLAYING).unwrap_err();
    assert!(err.starts_with("/nonexistent/proc: "), "{err}");
}

#[test]
fn finds_the_display() {
    assert_eq!(
        display(b"HOME=/home/me\0DISPLAY=:1\0SteamAppId=0\0").as_deref(),
        Some(":1")
    );
    assert_eq!(display(b"DISPLAY=:1").as_deref(), Some(":1"));
}

#[test]
fn no_display() {
    for environ in [
        &b""[..],
        b"HOME=/home/me\0",
        b"DISPLAY=\0",
        b"WAYLAND_DISPLAY=wayland-1\0",
    ] {
        assert_eq!(display(environ), None, "{environ:?}");
    }
}

#[test]
fn reads_the_apps_display() {
    let proc = std::env::temp_dir().join(format!("switcher-proc-display-{}", std::process::id()));
    let _ = fs::remove_dir_all(&proc);
    let add = |name: &str, cmdline: &[u8], environ: &[u8]| {
        fs::create_dir_all(proc.join(name)).unwrap();
        fs::write(proc.join(name).join("cmdline"), cmdline).unwrap();
        fs::write(proc.join(name).join("environ"), environ).unwrap();
    };
    add("1", &cmdline(&["/sbin/init"]), b"DISPLAY=:0\0");
    let err = display_of_in(&proc, NOW_PLAYING).unwrap_err();
    assert_eq!(err, "app 2457742405 isn't running");

    add(
        "4652",
        &reaper("AppId=2457742405", "now-playing"),
        b"HOME=/home/me\0",
    );
    let err = display_of_in(&proc, NOW_PLAYING).unwrap_err();
    assert_eq!(err, "app 2457742405 has no DISPLAY");

    add(
        "4652",
        &reaper("AppId=2457742405", "now-playing"),
        b"DISPLAY=:1\0",
    );
    assert_eq!(display_of_in(&proc, NOW_PLAYING).unwrap(), ":1");
    fs::remove_dir_all(&proc).unwrap();
}
