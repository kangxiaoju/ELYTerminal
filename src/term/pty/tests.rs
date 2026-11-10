use alacritty_terminal::{
    grid::Dimensions,
    index::{Column, Line},
};
use futures::StreamExt;

use super::*;
use crate::term::{drop::Quote, marks::Click};
use crate::term::{
    notify::Ask,
    status::{Record, State},
};

struct Ran {
    folders: Vec<PathBuf>,
    marks: Vec<Mark>,
    /// What programs said of their status, a full reset as none.
    statuses: Vec<Option<Status>>,
    /// Notifications and progress programs asked for.
    asks: Vec<Ask>,
    /// Titles, notifications and full resets, in the output's order.
    said: Vec<String>,
    text: String,
    /// The first line's cells that carry a prompt's link.
    prompt: Vec<usize>,
    success: bool,
    pid: u32,
    columns: usize,
}

fn run(script: &str, size: (usize, usize)) -> Ran {
    crate::testlog::init();
    let launch = Launch {
        program: Some(("/bin/sh".into(), vec!["-c".into(), script.into()])),
        cwd: std::env::temp_dir(),
        env: vec![("ELY".into(), "ok".into())],
        reports: false,
        quote: Quote::Posix,
    };
    let (pty, mut events) =
        spawn(&launch, size, (8, 16), config(&Settings::default())).expect("sh starts");
    let (folders, marks, statuses, asks, said, success) = futures::executor::block_on(async {
        let (mut folders, mut marks, mut statuses, mut asks, mut said, mut success) = (
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            false,
        );
        while let Some(event) = events.next().await {
            match event {
                PtyEvent::Folder(folder) => folders.push(folder),
                PtyEvent::Mark(mark, _) => marks.push(mark),
                PtyEvent::Status(status, _) => statuses.push(Some(status)),
                PtyEvent::Reset => {
                    said.push("reset".to_string());
                    statuses.push(None);
                }
                PtyEvent::Title(title) => said.push(format!("title {title}")),
                PtyEvent::Clipboard(packet) => said.push(format!("clipboard {packet:?}")),
                PtyEvent::Ask(ask, _) => {
                    if let Ask::Show(notice) = &ask {
                        said.push(format!("show {}", notice.title));
                    }
                    asks.push(ask);
                }
                PtyEvent::Term(Event::ChildExit(status)) => success = status.success(),
                PtyEvent::Term(Event::PtyWrite(reply)) => pty.write(reply.into_bytes()),
                PtyEvent::Term(Event::Exit) => break,
                PtyEvent::Term(_) | PtyEvent::Mouse(_) | PtyEvent::OutputProcessed(_) => {}
            }
        }
        (folders, marks, statuses, asks, said, success)
    });
    let term = pty.term.lock();
    let row = &term.grid()[Line(0)];
    let text: String = (0..24).map(|column| row[Column(column)].c).collect();
    let prompt = (0..24).filter(|&column| {
        row[Column(column)]
            .hyperlink()
            .is_some_and(|link| link.uri() == crate::term::prompts::LINK)
    });
    Ran {
        folders,
        marks,
        statuses,
        asks,
        said,
        text: text.trim_end().to_string(),
        prompt: prompt.collect(),
        success,
        pid: pty.pid,
        columns: term.columns(),
    }
}

#[test]
fn a_program_prints_into_the_grid_with_its_variables_and_id() {
    let ran = run("printf 'ely %s %s' \"$ELY\" $$", (40, 4));
    assert!(ran.folders.is_empty());
    assert_eq!(ran.text, format!("ely ok {}", ran.pid));
    assert!(ran.success);
}

#[test]
fn a_kitty_graphics_command_reaches_its_handler_at_the_cursor() {
    let ran = run(r"printf 'ab\033_Gi=1,a=q,q=2;AAAA\033\\cd'", (24, 4));
    assert_eq!(ran.text, "abcd", "the command prints nothing");
    let seen = format!("graphics {}: 16 bytes at 0, 2: i=1,a=q,q=2;AAAA", ran.pid);
    assert!(crate::testlog::logged(&seen), "{seen}");
    // A control inside the string's ST runs where it stands.
    let ran = run(
        r"printf 'ab\033_Gi=1,q=2\033\r\\cd\033_Gi=2,q=2\033\\'",
        (24, 4),
    );
    assert_eq!(ran.text, "cd");
    let seen = format!("graphics {}: 7 bytes at 0, 2: i=2,q=2", ran.pid);
    assert!(crate::testlog::logged(&seen), "{seen}");
}

#[test]
fn kitty_graphics_replies_reach_the_program_and_a_temporary_file_goes() {
    use base64::Engine;
    let file = std::env::temp_dir().join(format!("tty-graphics-protocol-{}", std::process::id()));
    std::fs::write(&file, [1, 2, 3]).expect("written");
    let path = base64::engine::general_purpose::STANDARD.encode(file.to_string_lossy().as_bytes());
    // Raw, unechoed; a lost reply ends the read soon.
    let script = format!(
        r#"stty raw -echo min 0 time 10; printf '\033_Gi=31,s=1,v=1,a=q,f=24;AAAA\033\\\033_Gi=32,t=t,f=24,s=1,v=1;{path}\033\\'; r=$(dd bs=1 count=24 2>/dev/null | tr '\033' E); stty sane; printf '%s' "$r""#
    );
    let ran = run(&script, (40, 4));
    assert_eq!(ran.text, r"E_Gi=31;OKE\E_Gi=32;OKE\");
    assert!(!file.exists(), "the temporary file was deleted");
}

/// Runs `script` and waits, as a host's view does, for the parser to take `target` raw bytes;
/// returns the grid's first row then, and when the program ended.
fn barriered(script: &str, target: u64) -> (String, String) {
    crate::testlog::init();
    let launch = Launch {
        program: Some(("/bin/sh".into(), vec!["-c".into(), script.into()])),
        cwd: std::env::temp_dir(),
        env: Vec::new(),
        reports: false,
        quote: Quote::Posix,
    };
    let (pty, mut events) =
        spawn(&launch, (40, 4), (8, 16), config(&Settings::default())).expect("sh starts");
    let row = |pty: &Pty| -> String {
        let term = pty.term.lock();
        let row = &term.grid()[Line(0)];
        let text: String = (0..40).map(|column| row[Column(column)].c).collect();
        text.trim_end().to_string()
    };
    let mut barrier = crate::term::barrier::Barrier::default();
    let mut at = barrier.arm(target, pty.listen_output());
    let mut seen = at.map(|_| row(&pty));
    futures::executor::block_on(async {
        while let Some(event) = events.next().await {
            match event {
                PtyEvent::OutputProcessed(bytes) => {
                    at = at.or(barrier.advance(bytes));
                    if at.is_some() && seen.is_none() {
                        seen = Some(row(&pty));
                    }
                }
                PtyEvent::Term(Event::PtyWrite(reply)) => pty.write(reply.into_bytes()),
                PtyEvent::Term(Event::Exit) => break,
                _ => {}
            }
        }
    });
    (seen.expect("the barrier fired"), row(&pty))
}

#[test]
fn the_barrier_fires_when_the_parser_has_drawn_the_counted_bytes_not_when_they_are_read() {
    // `late` comes a second after, so a grid holding it would mean the barrier waited for more.
    let (at, last) = barriered(r"printf visible; sleep 1; printf late", 7);
    assert_eq!(at, "visible");
    assert_eq!(last, "visiblelate");
}

#[test]
fn a_barrier_inside_a_synchronized_update_shows_the_last_complete_screen_and_does_not_stall() {
    // The update's 6 bytes after `one` and the 8 of its start are read, not drawn, until its end.
    let (at, last) = barriered(
        r"printf 'one\033[?2026hhidden'; sleep 1; printf '\033[?2026l'",
        3 + 8 + 6,
    );
    assert_eq!(at, "one");
    assert_eq!(last, "onehidden");
}

#[test]
fn a_barrier_inside_a_partial_sequence_fires_with_the_screen_before_it_and_the_stream_is_unchanged() {
    // Each script's first write ends inside a sequence, the target past its last byte; the parser
    // (or the prompt scanner, for the OSC) holds the rest a second. The barrier means "a complete screen",
    // not "every byte consumed", so it shows what came before, and the finished sequence still lands.
    // A UTF-8 character cut in two:
    let (at, last) = barriered(r"printf 'ab\303'; sleep 1; printf '\251c'", 3);
    assert_eq!((at.as_str(), last.as_str()), ("ab", "abéc"));
    // A CSI cut after its parameter:
    let (at, last) = barriered(r"printf 'ab\033[3'; sleep 1; printf '1mc'", 5);
    assert_eq!((at.as_str(), last.as_str()), ("ab", "abc"));
    // An OSC 133 mark the prompt scanner holds whole:
    let (at, last) = barriered(r"printf 'ab\033]133;A'; sleep 1; printf '\007c'", 9);
    assert_eq!((at.as_str(), last.as_str()), ("ab", "abc"));
    // An OSC the scanner passes on as it comes, cut before its terminator:
    let (at, last) = barriered(r"printf 'ab\033]0;title'; sleep 1; printf '\007c'", 11);
    assert_eq!((at.as_str(), last.as_str()), ("ab", "abc"));
}

#[test]
fn a_barrier_at_zero_fires_at_once_and_the_program_s_own_queries_are_still_answered() {
    let (at, _) = barriered("true", 0);
    assert_eq!(at, "");
    // Raw, unechoed: the program asks for a mode and reads the parser's reply as before.
    let (_, last) = barriered(
        r#"stty raw -echo min 0 time 10; printf '\033[?25$p'; r=$(dd bs=1 count=10 2>/dev/null | tr '\033' E); stty sane; printf '%s' "$r""#,
        0,
    );
    assert_eq!(last, r"E[?25;1$y");
}

#[test]
fn osc_7_reaches_the_session_as_a_folder() {
    let ran = run(
        "printf '\\033]7;file:///tmp/a%%20b\\007x\\033]7;file://elsewhere.invalid/c\\033\\\\'",
        (40, 4),
    );
    assert_eq!(ran.folders, [PathBuf::from("/tmp/a b")]);
    assert_eq!(ran.text, "x");
}

#[test]
fn osc_133_reaches_the_session_as_marks() {
    let ran = run(
        "printf '\\033]133;A\\007>\\033]133;B\\007\\033]133;C\\033\\\\x\\033]133;D;3\\007'",
        (40, 4),
    );
    let marks = [
        Mark::Prompt(Click::Arrows),
        Mark::Input,
        Mark::Output,
        Mark::Done(Some(3)),
    ];
    assert_eq!(ran.marks, marks);
    assert_eq!(ran.text, ">x");
    assert_eq!(ran.prompt, [0], "the prompt's cell, not the output's");
}

#[test]
fn osc_7501_and_a_full_reset_reach_the_session_in_order() {
    let ran = run(
        "printf '\\033]7501;state=working:progress=40\\033\\\\\\033]7501;?\\007\\033cx'",
        (40, 4),
    );
    let working = Record {
        state: State::Working,
        kind: None,
        progress: Some(40),
        app: None,
        title: None,
        msg: None,
    };
    let said = [
        Some(Status::Report(String::new(), working)),
        Some(Status::Query),
        None,
    ];
    assert_eq!(ran.statuses, said);
    assert_eq!(ran.text, "x");
}

#[test]
fn notifications_and_progress_reach_the_session_in_order() {
    let ran = run(
        "printf '\\033]9;Hello\\007\\033]777;notify;T;B\\007\\033]99;i=a:d=0;Hel\\033\\\\\\033]99;i=a;lo\\033\\\\\\033]9;4;1;40\\007x'",
        (40, 4),
    );
    let titles: Vec<String> = ran
        .asks
        .iter()
        .map(|ask| match ask {
            Ask::Show(notice) => format!("{}/{}", notice.title, notice.body),
            other => format!("{other:?}"),
        })
        .collect();
    assert_eq!(
        titles,
        ["Hello/", "T/B", "Hello/", "Progress(Set(Some(40)))"]
    );
    assert_eq!(ran.text, "x");
}

#[test]
fn osc_99_gathers_a_full_encoded_chunk_and_the_lines_base64_carries() {
    let ran = run(
        "printf '\\033]99;i=m:d=0:e=1;'; yes YWFh | head -n 1024 | tr -d '\\n'; printf '\\033\\\\\\033]99;i=m:d=0:e=1;RU5E\\033\\\\\\033]99;i=m:p=body:e=1;TGluZSAxCkxpbmUgMg==\\007x'",
        (40, 4),
    );
    let [Ask::Show(notice)] = ran.asks.as_slice() else {
        panic!("one notification: {:?}", ran.asks);
    };
    assert_eq!(notice.title, format!("{}END", "a".repeat(3072)));
    assert_eq!(notice.body, "Line 1\nLine 2");
    assert_eq!(ran.text, "x");
}

#[test]
fn titles_reach_the_session_in_their_place_among_notifications() {
    let long = "y".repeat(5000);
    let many: Vec<String> = (1..=17).map(|at| at.to_string()).collect();
    let parts = [
        "\\033]2;Old\\007".to_string(),
        "\\033[22t".into(),
        "\\033]2;New\\007".into(),
        "\\033]9;a\\007".into(),
        "\\033[23t".into(),
        "\\033]9;b\\007".into(),
        "\\033c".into(),
        "\\033]9;c\\007".into(),
        format!("\\033]2;{long}\\007"),
        format!("\\033]2;{}\\007", many.join(";")),
        "\\033]9;d\\007".into(),
    ];
    let whole = run(&format!("printf '{}x'", parts.concat()), (40, 4));
    let split: Vec<String> = parts
        .iter()
        .map(|part| format!("printf '{part}'; sleep 0.1"))
        .collect();
    let split = run(&format!("{}; printf x", split.join("; ")), (40, 4));
    let said = [
        "title Old".to_string(),
        "title New".into(),
        "show a".into(),
        "title Old".into(),
        "show b".into(),
        "reset".into(),
        "show c".into(),
        format!("title {long}"),
        format!("title {}", many[..15].join(";")),
        "show d".into(),
    ];
    assert_eq!(whole.said, said, "one write");
    assert_eq!(split.said, said, "a write a sequence");
    assert_eq!((whole.text.as_str(), split.text.as_str()), ("x", "x"));
}

#[test]
fn a_long_escape_of_controls_keeps_the_pty_reading() {
    let ran = run(
        "printf '\\033'; head -c 3000000 /dev/zero; printf '[0mREADY'",
        (40, 4),
    );
    assert_eq!(ran.text, "READY");
}

#[test]
fn the_foreground_is_the_program_s_own_group() {
    let launch = Launch {
        program: Some(("/bin/sh".into(), vec!["-c".into(), "exec sleep 30".into()])),
        cwd: std::env::temp_dir(),
        env: Vec::new(),
        reports: false,
        quote: Quote::Posix,
    };
    let (pty, _events) =
        spawn(&launch, (20, 4), (8, 16), config(&Settings::default())).expect("sh starts");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while pty.foreground().ok().flatten() != Some(pty.pid as i32) {
        assert!(
            std::time::Instant::now() < deadline,
            "{:?}",
            pty.foreground()
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

#[test]
fn a_size_past_the_kernel_is_one_size_for_grid_and_program() {
    let ran = run("stty size", (9000, 8));
    assert_eq!(ran.columns, usize::from(u16::MAX / 8));
    assert_eq!(ran.text, format!("8 {}", u16::MAX / 8));
}

/// Starts `script`, waits for it to print READY, closes it, and waits for the reap's log.
fn close_after_ready(script: &str, reaped: &str) {
    crate::testlog::init();
    let launch = Launch {
        program: Some(("/bin/sh".into(), vec!["-c".into(), script.into()])),
        cwd: std::env::temp_dir(),
        env: Vec::new(),
        reports: false,
        quote: Quote::Posix,
    };
    let (pty, mut events) =
        spawn(&launch, (20, 4), (8, 16), config(&Settings::default())).expect("sh starts");
    futures::executor::block_on(async {
        while let Some(event) = events.next().await {
            let term = pty.term.lock();
            let text: String = (0..5)
                .map(|column| term.grid()[Line(0)][Column(column)].c)
                .collect();
            drop(term);
            if matches!(event, PtyEvent::Term(Event::Wakeup)) && text == "READY" {
                break;
            }
        }
    });
    let pid = pty.pid;
    drop(pty);
    let hung_up = format!("pty {pid}: hung up, {reaped}");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !crate::testlog::logged(&hung_up) {
        assert!(std::time::Instant::now() < deadline, "no `{hung_up}`");
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(crate::testlog::logged(&format!("pty {pid}: closing")));
    let alive = unsafe { libc::kill(pid as libc::pid_t, 0) } == 0;
    assert!(!alive, "the program was reaped");
}

#[test]
fn closing_a_session_hangs_up_its_program() {
    close_after_ready("printf READY; exec cat", "signal: 1 (SIGHUP)");
}

#[test]
fn a_program_that_ignores_sighup_ends_when_the_pty_closes() {
    // cat reads EOF on macOS and EIO on Linux; the shell's own exit holds on both.
    close_after_ready("trap '' HUP; printf READY; cat; exit 7", "exit status: 7");
}

#[test]
fn a_missing_folder_stops_the_start() {
    let launch = Launch {
        program: None,
        cwd: "/nonexistent/ely".into(),
        env: Vec::new(),
        reports: false,
        quote: Quote::Posix,
    };
    let error = spawn(&launch, (20, 4), (8, 16), config(&Settings::default()))
        .err()
        .expect("a missing folder is an error");
    assert!(error.to_string().contains("/nonexistent/ely"));
}

#[test]
fn a_folder_that_cannot_be_entered_stops_the_start() {
    use std::os::unix::fs::PermissionsExt;
    let folder = std::env::temp_dir().join(format!("elyterminal-shut-{}", std::process::id()));
    std::fs::create_dir_all(&folder).expect("a temp folder");
    std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o600)).expect("600");
    let launch = Launch {
        program: Some(("/bin/sh".into(), vec!["-c".into(), "true".into()])),
        cwd: folder.clone(),
        env: Vec::new(),
        reports: false,
        quote: Quote::Posix,
    };
    let started = spawn(&launch, (20, 4), (8, 16), config(&Settings::default()));
    std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o700)).expect("700");
    std::fs::remove_dir(&folder).expect("the temp folder goes");
    let error = started.err().expect("a folder it cannot enter is an error");
    let error = error.to_string();
    assert!(error.contains("Permission denied"), "{error}");
    assert!(error.contains(&folder.display().to_string()), "{error}");
}

#[test]
fn sizes_hold_at_least_two_by_one_and_cap_at_the_kernel() {
    let size = fit(70_000, 24, (8, 16));
    assert_eq!((size.num_cols, size.num_lines), (u16::MAX / 8, 24));
    let size = fit(0, 0, (8, 16));
    assert_eq!((size.num_cols, size.num_lines), (2, 1));
}
