use serea_android_bridge::{BridgeError, close, initialize};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::Duration,
};

#[test]
fn ownership_child() {
    let Ok(path) = std::env::var("SEREA_TEST_CHILD_DB") else {
        return;
    };
    let ready = std::env::var("SEREA_TEST_CHILD_READY").unwrap();
    let handle = initialize(path.clone()).unwrap();
    let _wal_connection = if std::env::var_os("SEREA_TEST_WAL").is_some() {
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=0; CREATE TABLE synthetic_recovery (value TEXT NOT NULL); INSERT INTO synthetic_recovery VALUES ('committed-before-kill');").unwrap();
        Some(conn)
    } else {
        None
    };
    fs::write(ready, b"ready").unwrap();
    let mut input = [0; 1];
    std::io::stdin().read_exact(&mut input).unwrap();
    close(handle).unwrap();
}

struct Owner(Child);
impl Drop for Owner {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

fn spawn_owner(db: &Path, ready: &Path) -> Owner {
    spawn_owner_with_wal(db, ready, false)
}

fn spawn_owner_with_wal(db: &Path, ready: &Path, wal: bool) -> Owner {
    let mut command = Command::new(std::env::current_exe().unwrap());
    if wal {
        command.env("SEREA_TEST_WAL", "1");
    }
    let child = command
        .args(["--exact", "ownership_child", "--nocapture"])
        .env("SEREA_TEST_CHILD_DB", db)
        .env("SEREA_TEST_CHILD_READY", ready)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    let mut owner = Owner(child);
    for _ in 0..1500 {
        if ready.exists() {
            return owner;
        }
        assert!(
            owner.0.try_wait().unwrap().is_none(),
            "child exited before acquiring ownership"
        );
        thread::sleep(Duration::from_millis(20));
    }
    panic!("child timed out before acquiring ownership")
}

fn db_path(path: &Path) -> String {
    path.to_str().unwrap().to_owned()
}

static TEST_OWNER: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn distinct_processes_exclude_same_db_and_release_on_close_or_kill() {
    let _guard = TEST_OWNER.lock().unwrap();
    let mut random = [0_u8; 8];
    getrandom::fill(&mut random).unwrap();
    let unique = u64::from_ne_bytes(random);
    let dir: PathBuf =
        std::env::temp_dir().join(format!("serea-owner-{}-{unique}", std::process::id()));
    fs::create_dir(&dir).unwrap();
    let db = dir.join("one.db");
    let other = dir.join("other.db");
    let reserved = dir.join("one.db.serea-owner-lock");
    assert_eq!(
        initialize(db_path(&reserved)),
        Err(BridgeError::StorageUnavailable)
    );
    assert!(!reserved.exists());
    for suffix in ["-wal", "-shm", "-journal"] {
        let reserved = dir.join(format!("one.db{suffix}"));
        assert_eq!(
            initialize(db_path(&reserved)),
            Err(BridgeError::StorageUnavailable)
        );
        assert!(!reserved.exists());
    }
    let mut owner = spawn_owner(&db, &dir.join("ready1"));
    assert!(
        matches!(
            initialize(db_path(&db)),
            Err(BridgeError::StorageUnavailable)
        ),
        "another process must not open the owned DB"
    );
    #[cfg(unix)]
    {
        let alias = dir.join("alias.db");
        std::os::unix::fs::symlink(&db, &alias).unwrap();
        assert_eq!(
            initialize(db_path(&alias)),
            Err(BridgeError::StorageUnavailable)
        );
        let hardlink = dir.join("hardlink.db");
        fs::hard_link(&db, &hardlink).unwrap();
        assert_eq!(
            initialize(db_path(&hardlink)),
            Err(BridgeError::StorageUnavailable)
        );
        fs::remove_file(hardlink).unwrap();
    }
    let h = initialize(db_path(&other)).unwrap();
    close(h).unwrap();
    owner.0.stdin.take().unwrap().write_all(b"x").unwrap();
    assert!(owner.0.wait().unwrap().success());
    let h = initialize(db_path(&db)).unwrap();
    close(h).unwrap();
    let mut owner = spawn_owner(&db, &dir.join("ready2"));
    owner.0.kill().unwrap();
    owner.0.wait().unwrap();
    let h = initialize(db_path(&db)).unwrap();
    close(h).unwrap();
    assert_eq!(close(h), Err(BridgeError::InvalidHandle));

    let invalid = dir.join("invalid.db");
    let content = b"not a sqlite database";
    fs::write(&invalid, content).unwrap();
    assert!(initialize(db_path(&invalid)).is_err());
    assert_eq!(fs::read(&invalid).unwrap(), content);
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(dir.join("invalid.db.serea-owner-lock"))
        .unwrap();
    lock.try_lock()
        .expect("failed initialize must release ownership");
}

#[test]
fn committed_wal_survives_kill_and_core_recovery() {
    let _guard = TEST_OWNER.lock().unwrap();
    let mut random = [0_u8; 8];
    getrandom::fill(&mut random).unwrap();
    let dir = std::env::temp_dir().join(format!(
        "serea-wal-{}-{}",
        std::process::id(),
        u64::from_ne_bytes(random)
    ));
    fs::create_dir(&dir).unwrap();
    let db = dir.join("synthetic.db");
    let mut owner = spawn_owner_with_wal(&db, &dir.join("ready"), true);
    assert!(dir.join("synthetic.db-wal").metadata().unwrap().len() > 32);
    owner.0.kill().unwrap();
    owner.0.wait().unwrap();
    let handle = initialize(db_path(&db)).unwrap();
    let conn = rusqlite::Connection::open(&db).unwrap();
    let value: String = conn
        .query_row("SELECT value FROM synthetic_recovery", [], |r| r.get(0))
        .unwrap();
    assert_eq!(value, "committed-before-kill");
    let integrity: String = conn
        .query_row("PRAGMA integrity_check", [], |r| r.get(0))
        .unwrap();
    assert_eq!(integrity, "ok");
    drop(conn);
    close(handle).unwrap();
    let handle = initialize(db_path(&db)).unwrap();
    close(handle).unwrap();
}
