// SPDX-License-Identifier: GPL-3.0-only
//! The applet's session token. All of the applet's processes keep it and
//! renew it together.
//!
//! The panel runs one applet process per side of each panel: four on a panel
//! spread over two monitors. They give the daemon one client name, so they
//! share one token. When the daemon stops honoring it (the applet's binary was
//! replaced under them, or the daemon lost its sessions), all of them find out
//! within a moment of each other. super-engine's `session::obtain` keeps its
//! lock and its cache inside the process, so each one asked the daemon for a
//! new token on its own. Each request from a binary the daemon can't vouch for
//! (and a binary replaced on disk is one) raises a consent popup, so one
//! `just install` raised four.
//!
//! So asking goes through a lock file in `$XDG_RUNTIME_DIR`. The first process
//! that needs a token takes the lock, asks, records the answer and lets go.
//! The others wait for the lock and find that answer: the new token, or the
//! reason the daemon refused one, which they take as their own instead of
//! asking again.
//!
//! Answers are recorded by binary: the path the kernel gives for
//! `/proc/self/exe`, which is what the daemon binds a token to. The daemon
//! revokes a token as soon as a different binary presents it, so a process
//! only takes a token minted for its own binary. During an upgrade, the old
//! processes (whose binary now reads `… (deleted)`) and the new ones each get
//! one token, and they never revoke each other's.
//!
//! The record lives only as long as the login, as the runtime directory does.
//! The token the next login starts from is in super-engine's token file, as
//! before. That file has no way to say which binary its token is for, so a
//! process only starts from it while it has never had a token refused.

use std::collections::{BTreeMap, HashSet};
use std::fs::{self, DirBuilder, File, OpenOptions};
use std::future::Future;
use std::io;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex, MutexGuard};

use log::{info, warn};
use serde::{Deserialize, Serialize};
use super_engine_client::http_client::{self, HttpError, HttpResult};
use super_engine_client::session as engine_session;

use crate::features::{APP_NAME, CLIENT_NAME, Feature};

/// The lock a process holds while it decides on a token, and while it asks
/// for one.
const LOCK_FILE: &str = "session.lock";

/// What the daemon answered, by binary: [`Answers`].
const ANSWERS_FILE: &str = "session.json";

/// The latest answer the daemon gave processes running one binary.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
struct Answer {
    /// How many times one of them has asked. If this goes up while a process
    /// waits for the lock, the ask that raised it answers that process too.
    asked: u64,
    /// The token the latest successful ask got.
    token: Option<String>,
    /// Why the latest ask was refused, when it was.
    denied: Option<String>,
}

/// The daemon's answers since login, by binary.
type Answers = BTreeMap<String, Answer>;

/// The token one process presents, and the tokens it has seen refused.
#[derive(Debug, Default)]
struct Held {
    /// The token this process presents until the daemon refuses it.
    current: Option<String>,
    /// Every token the daemon has refused this process. Tokens are never
    /// reused, so a refused one is never worth presenting again.
    refused: HashSet<String>,
}

/// What a process that needs a token does once it holds the lock.
#[derive(Debug, PartialEq, Eq)]
enum Next {
    /// Present this token without asking.
    Use(String),
    /// A process running this binary asked while this one waited, and the
    /// daemon refused it for this reason. This process takes that answer as
    /// its own.
    Refused(String),
    /// Ask the daemon.
    Ask,
}

/// What a process running `exe` does for a token: `before` is the record as
/// it stood when the process started waiting for the lock, `now` as it stands
/// under the lock, and `stored` the token in super-engine's token file.
fn next(exe: &str, before: &Answers, now: &Answers, stored: Option<String>, held: &Held) -> Next {
    let mine = now.get(exe);

    // A token minted for this binary that this process hasn't seen refused:
    // the one another process just asked for, usually.
    if let Some(token) = mine
        .and_then(|a| a.token.as_ref())
        .filter(|t| !held.refused.contains(*t))
    {
        return Next::Use(token.clone());
    }

    // A request this binary made while this process waited was refused. The
    // daemon would answer this process the same way, and a dismissed popup
    // shown again for each process is the problem this module exists to
    // solve.
    let asked_before = before.get(exe).map_or(0, |a| a.asked);
    if let Some(Answer {
        asked,
        denied: Some(reason),
        ..
    }) = mine
        && *asked > asked_before
    {
        return Next::Refused(reason.clone());
    }

    // A process that has never had a token refused starts from the stored
    // one, as a fresh login does. Not if it is recorded as another binary's,
    // though: presenting it would get it revoked from under that binary's
    // processes.
    if held.refused.is_empty()
        && let Some(token) = stored
        && !now
            .iter()
            .any(|(binary, a)| binary != exe && a.token.as_ref() == Some(&token))
    {
        return Next::Use(token);
    }

    Next::Ask
}

/// One process's token, and the files it shares with the applet's other
/// processes.
struct Session {
    /// The directory that holds the lock and the answers. `None` when there
    /// is no runtime directory: each process then asks on its own.
    dir: Option<PathBuf>,
    held: Mutex<Held>,
    /// Lets one task in this process through to the lock at a time. The
    /// others wait here, and find the token it got in `held`.
    asking: tokio::sync::Mutex<()>,
}

impl Session {
    fn new(dir: Option<PathBuf>) -> Self {
        Self {
            dir,
            held: Mutex::new(Held::default()),
            asking: tokio::sync::Mutex::new(()),
        }
    }

    fn held(&self) -> MutexGuard<'_, Held> {
        // The state is a token and a set of tokens, whole after any panic.
        self.held
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// The token to present for a process running `exe`. The daemon is asked,
    /// through `ask`, only when no process running this binary got a token
    /// that this process hasn't seen refused. `stored` reads super-engine's
    /// token file.
    async fn token<Ask, Fut>(
        &self,
        exe: &str,
        stored: impl FnOnce() -> Option<String>,
        ask: Ask,
    ) -> HttpResult<String>
    where
        Ask: FnOnce() -> Fut,
        Fut: Future<Output = HttpResult<String>>,
    {
        if let Some(token) = self.held().current.clone() {
            return Ok(token);
        }
        // Read before any wait, the one for this process's other tasks
        // included, so that every request made during the wait counts as one.
        let before = self.read_answers();
        let _asking = self.asking.lock().await;
        if let Some(token) = self.held().current.clone() {
            return Ok(token);
        }
        let _lock = self.lock().await;
        let mut answers = self.read_answers();

        let decision = next(exe, &before, &answers, stored(), &self.held());
        match decision {
            Next::Use(token) => {
                let mut held = self.held();
                if !held.refused.is_empty() {
                    info!("Using the session token another applet process got");
                }
                held.current = Some(token.clone());
                Ok(token)
            }
            Next::Refused(reason) => {
                info!(
                    "Not asking for a session token: the request made while this one \
                     waited was refused ({reason})"
                );
                Err(HttpError::AuthDenied { reason })
            }
            Next::Ask => {
                let result = ask().await;
                let answer = answers.entry(exe.to_owned()).or_default();
                answer.asked += 1;
                answer.denied = None;
                match &result {
                    Ok(token) => {
                        answer.token = Some(token.clone());
                        self.held().current = Some(token.clone());
                    }
                    Err(HttpError::AuthDenied { reason }) => answer.denied = Some(reason.clone()),
                    // The daemon didn't answer. Whoever asks next asks again.
                    Err(_) => {}
                }
                self.write_answers(&answers);
                result
            }
        }
    }

    /// The daemon refused `token`. Stop presenting it, and never take it up
    /// again.
    fn refused(&self, token: &str) {
        let mut held = self.held();
        // Another task may have replaced it already. Keep that one.
        if held.current.as_deref() == Some(token) {
            held.current = None;
        }
        held.refused.insert(token.to_owned());
    }

    /// Wait for the lock and take it. It is held until the file is dropped.
    /// `None` when there is nowhere to keep it, and then the process decides
    /// on its own.
    async fn lock(&self) -> Option<File> {
        let dir = self.dir.clone()?;
        let locked = tokio::task::spawn_blocking(move || -> io::Result<File> {
            make_dir(&dir)?;
            let file = OpenOptions::new()
                .create(true)
                .truncate(false)
                .write(true)
                .mode(0o600)
                .open(dir.join(LOCK_FILE))?;
            // Blocks for as long as another process is asking, which may be
            // as long as the person takes to answer its consent popup.
            file.lock()?;
            Ok(file)
        })
        .await
        .unwrap_or_else(|e| Err(io::Error::other(e)));
        locked
            .inspect_err(|e| warn!("Can't take the session lock, asking without it: {e}"))
            .ok()
    }

    /// The answers on record. None on record, or none that read, is none.
    fn read_answers(&self) -> Answers {
        self.dir
            .as_ref()
            .and_then(|dir| fs::read_to_string(dir.join(ANSWERS_FILE)).ok())
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    /// Record `answers`. They hold tokens, so only this user can read them.
    /// The new file replaces the old one in one step, so a reader that
    /// doesn't hold the lock never sees half of it.
    fn write_answers(&self, answers: &Answers) {
        let Some(dir) = &self.dir else { return };
        let write = || -> io::Result<()> {
            make_dir(dir)?;
            let text = serde_json::to_vec(answers).map_err(io::Error::other)?;
            let partial = dir.join(format!("{ANSWERS_FILE}.partial"));
            {
                use std::io::Write as _;
                let mut file = OpenOptions::new()
                    .create(true)
                    .truncate(true)
                    .write(true)
                    .mode(0o600)
                    .open(&partial)?;
                file.write_all(&text)?;
            }
            fs::rename(&partial, dir.join(ANSWERS_FILE))
        };
        if let Err(e) = write() {
            warn!(
                "Can't record the session token for the applet's other processes: {e}. \
                 Each of them will ask for its own."
            );
        }
    }
}

/// Create `dir`, which only this user may enter.
fn make_dir(dir: &Path) -> io::Result<()> {
    DirBuilder::new().recursive(true).mode(0o700).create(dir)
}

/// This process's token, and the files its siblings share, under
/// `$XDG_RUNTIME_DIR/super-cosmic-applet/`. Both features share it, because
/// they share the daemon and the token: see
/// `features::tests::both_features_share_the_daemon_and_the_token`.
static SESSION: LazyLock<Session> = LazyLock::new(|| {
    let dir = std::env::var_os("XDG_RUNTIME_DIR")
        .filter(|dir| !dir.is_empty())
        .map(|dir| PathBuf::from(dir).join(CLIENT_NAME));
    if dir.is_none() {
        warn!("No XDG_RUNTIME_DIR: each applet process will ask for its own session token");
    }
    Session::new(dir)
});

/// The binary this process is running, as the daemon sees it: where
/// `/proc/self/exe` points, with ` (deleted)` once the binary has been
/// replaced on disk.
fn this_binary() -> String {
    fs::read_link("/proc/self/exe").map_or_else(
        |_| String::new(),
        |path| path.to_string_lossy().into_owned(),
    )
}

/// A token for `product`'s daemon. It comes from this process, from another
/// applet process, or from asking the daemon, which may show a consent popup.
///
/// # Errors
///
/// The daemon refused a token, or couldn't be reached to ask for one.
pub async fn token(product: Feature) -> HttpResult<String> {
    SESSION
        .token(
            &this_binary(),
            || engine_session::load(product.app_id()),
            || ask(product),
        )
        .await
}

/// Ask `product`'s daemon for a token, and store it for the next login.
async fn ask(product: Feature) -> HttpResult<String> {
    info!("Asking {} for a session token", product.display_name());
    let auth = http_client::auth_request(product.socket_path(), APP_NAME, product.scopes()).await?;
    if let Err(e) = engine_session::save(
        product.app_id(),
        &auth.session_token,
        product.scopes(),
        &auth.scopes,
    ) {
        // This login still has it. The next one starts by asking again.
        warn!("Can't store the session token: {e}");
    }
    Ok(auth.session_token)
}

/// The daemon refused `token`: don't present it again.
pub fn refused(token: &str) {
    SESSION.refused(token);
}

/// Run `op` with a token, and once more with a new one if the daemon refuses
/// the first.
///
/// # Errors
///
/// `op`'s error, or the error from getting a token.
pub async fn with_token<T, F, Fut>(product: Feature, op: F) -> HttpResult<T>
where
    F: Fn(String) -> Fut,
    Fut: Future<Output = HttpResult<T>>,
{
    let first = token(product).await?;
    match op(first.clone()).await {
        Err(e) if e.is_invalid_session() || e.is_scope_denied() => {
            refused(&first);
            op(token(product).await?).await
        }
        result => result,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::future::join_all;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    const OLD: &str = "/usr/local/bin/super-cosmic-applet (deleted)";
    const NEW: &str = "/usr/local/bin/super-cosmic-applet";

    /// A directory for one test, removed when it ends.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "super-cosmic-applet-session-{name}-{}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&dir);
            Self(dir)
        }

        /// One applet process, sharing this directory with the others.
        fn process(&self) -> Session {
            Session::new(Some(self.0.clone()))
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// The daemon's side: counts the requests it gets, and either grants
    /// each one a new token, which it stores as super-engine's token file
    /// would, or refuses them all.
    #[derive(Clone, Default)]
    struct Daemon {
        asked: Arc<AtomicUsize>,
        stored: Arc<Mutex<Option<String>>>,
        refuse: Option<&'static str>,
    }

    impl Daemon {
        fn refusing(reason: &'static str) -> Self {
            Self {
                refuse: Some(reason),
                ..Self::default()
            }
        }

        fn storing(token: &str) -> Self {
            let daemon = Self::default();
            *daemon.stored.lock().unwrap() = Some(token.to_owned());
            daemon
        }

        fn asked(&self) -> usize {
            self.asked.load(Ordering::SeqCst)
        }

        /// `process`, running `exe`, getting a token.
        async fn token(&self, process: &Session, exe: &str) -> HttpResult<String> {
            let stored = self.stored.clone();
            process
                .token(exe, || stored.lock().unwrap().clone(), || self.ask())
                .await
        }

        async fn ask(&self) -> HttpResult<String> {
            let n = self.asked.fetch_add(1, Ordering::SeqCst) + 1;
            // Answering takes a while: long enough for the other processes
            // to line up behind the lock.
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            if let Some(reason) = self.refuse {
                return Err(HttpError::AuthDenied {
                    reason: reason.to_owned(),
                });
            }
            let token = format!("T{n}");
            *self.stored.lock().unwrap() = Some(token.clone());
            Ok(token)
        }

        /// Every process in `processes`, running `exe`, getting a token at
        /// the same moment.
        async fn tokens(&self, processes: &[Session], exe: &str) -> Vec<HttpResult<String>> {
            join_all(processes.iter().map(|p| self.token(p, exe))).await
        }
    }

    fn ok(results: Vec<HttpResult<String>>) -> Vec<String> {
        results.into_iter().map(|r| r.expect("a token")).collect()
    }

    /// What happened on 2026-11-19: `just install` replaced the binary under
    /// four running processes, and each of them asked for, and got a popup
    /// for, a token of its own.
    #[tokio::test(flavor = "multi_thread")]
    async fn processes_whose_token_is_revoked_together_ask_once() {
        let scratch = Scratch::new("revoked-together");
        let daemon = Daemon::storing("T0");
        let processes: Vec<_> = (0..4).map(|_| scratch.process()).collect();
        assert_eq!(ok(daemon.tokens(&processes, NEW).await), ["T0"; 4]);

        // The binary is replaced, and the daemon revokes T0 from all four.
        for process in &processes {
            process.refused("T0");
        }
        let tokens = ok(daemon.tokens(&processes, OLD).await);

        assert_eq!(daemon.asked(), 1, "one request, so one popup");
        assert_eq!(tokens, ["T1"; 4]);
    }

    /// Then the panel restarted them, and the new processes found the token
    /// the old binary was given. The daemon revoked it from each of them,
    /// and each asked again.
    #[tokio::test(flavor = "multi_thread")]
    async fn new_processes_dont_take_the_old_binarys_token() {
        let scratch = Scratch::new("restart");
        let daemon = Daemon::storing("T0");
        let old: Vec<_> = (0..4).map(|_| scratch.process()).collect();
        daemon.tokens(&old, NEW).await;
        for process in &old {
            process.refused("T0");
        }
        daemon.tokens(&old, OLD).await;
        assert_eq!(daemon.stored.lock().unwrap().as_deref(), Some("T1"));

        let new: Vec<_> = (0..4).map(|_| scratch.process()).collect();
        let tokens = ok(daemon.tokens(&new, NEW).await);

        assert_eq!(daemon.asked(), 2, "one request for each binary");
        assert_eq!(tokens, ["T2"; 4]);
    }

    /// The old and new processes may run side by side for a while. Each
    /// binary gets one token, and neither takes the other's: presenting it
    /// would get it revoked, and the two would take turns asking.
    #[tokio::test(flavor = "multi_thread")]
    async fn two_binaries_each_ask_once_and_keep_their_own() {
        let scratch = Scratch::new("two-binaries");
        let daemon = Daemon::default();
        let old: Vec<_> = (0..2).map(|_| scratch.process()).collect();
        let new: Vec<_> = (0..2).map(|_| scratch.process()).collect();

        let (old_tokens, new_tokens) =
            futures_util::join!(daemon.tokens(&old, OLD), daemon.tokens(&new, NEW));
        let (old_tokens, new_tokens) = (ok(old_tokens), ok(new_tokens));

        assert_eq!(daemon.asked(), 2);
        assert_eq!(old_tokens[0], old_tokens[1]);
        assert_eq!(new_tokens[0], new_tokens[1]);
        assert_ne!(old_tokens[0], new_tokens[0]);
    }

    /// The first install has no token anywhere. The four processes the panel
    /// starts ask once between them.
    #[tokio::test(flavor = "multi_thread")]
    async fn processes_starting_with_no_token_ask_once() {
        let scratch = Scratch::new("first-start");
        let daemon = Daemon::default();
        let processes: Vec<_> = (0..4).map(|_| scratch.process()).collect();

        assert_eq!(ok(daemon.tokens(&processes, NEW).await), ["T1"; 4]);
        assert_eq!(daemon.asked(), 1);
    }

    /// A popup dismissed once is not shown again for each process waiting
    /// behind it. They take the answer the first one got.
    #[tokio::test(flavor = "multi_thread")]
    async fn processes_waiting_on_a_refused_request_share_the_refusal() {
        let scratch = Scratch::new("dismissed");
        let daemon = Daemon::refusing("user_dismissed");
        let processes: Vec<_> = (0..4).map(|_| scratch.process()).collect();

        for result in daemon.tokens(&processes, NEW).await {
            assert!(
                matches!(&result, Err(HttpError::AuthDenied { reason }) if reason == "user_dismissed"),
                "{result:?}"
            );
        }
        assert_eq!(daemon.asked(), 1);
    }

    /// A refusal is only the answer for the processes that waited on it. The
    /// next try asks again: the popup may get an answer this time.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_refusal_only_answers_who_waited_for_it() {
        let scratch = Scratch::new("refused-then-retry");
        let daemon = Daemon::refusing("user_dismissed");
        let process = scratch.process();
        assert!(daemon.token(&process, NEW).await.is_err());
        assert!(daemon.token(&process, NEW).await.is_err());

        assert_eq!(daemon.asked(), 2);
    }

    /// Two tasks of one process (the two features' subscriptions) need a
    /// token together. They ask once.
    #[tokio::test(flavor = "multi_thread")]
    async fn tasks_of_one_process_ask_once() {
        let scratch = Scratch::new("one-process");
        let daemon = Daemon::default();
        let process = scratch.process();

        let (a, b) = futures_util::join!(daemon.token(&process, NEW), daemon.token(&process, NEW));
        assert_eq!((a.unwrap(), b.unwrap()), ("T1".into(), "T1".into()));
        assert_eq!(daemon.asked(), 1);
    }

    /// A token in hand is presented without a look at the files.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_held_token_is_used_until_refused() {
        let scratch = Scratch::new("held");
        let daemon = Daemon::default();
        let process = scratch.process();
        let first = daemon.token(&process, NEW).await.unwrap();
        fs::remove_dir_all(&scratch.0).unwrap();

        assert_eq!(daemon.token(&process, NEW).await.unwrap(), first);
        process.refused(&first);
        assert_ne!(daemon.token(&process, NEW).await.unwrap(), first);
        assert_eq!(daemon.asked(), 2);
    }

    /// A refusal that arrives after the task that saw it first already got
    /// the next token leaves that token alone.
    #[test]
    fn a_late_refusal_keeps_the_newer_token() {
        let process = Session::new(None);
        process.held().current = Some("T2".into());
        process.refused("T1");
        assert_eq!(process.held().current.as_deref(), Some("T2"));
    }

    /// With no runtime directory, a process gets on alone, as before.
    #[tokio::test(flavor = "multi_thread")]
    async fn no_runtime_directory_asks_alone() {
        let daemon = Daemon::default();
        let process = Session::new(None);
        assert_eq!(daemon.token(&process, NEW).await.unwrap(), "T1");
    }

    fn answers(entries: &[(&str, Answer)]) -> Answers {
        entries
            .iter()
            .map(|(exe, a)| ((*exe).to_owned(), a.clone()))
            .collect()
    }

    fn granted(asked: u64, token: &str) -> Answer {
        Answer {
            asked,
            token: Some(token.into()),
            denied: None,
        }
    }

    fn denied(asked: u64, reason: &str) -> Answer {
        Answer {
            asked,
            token: None,
            denied: Some(reason.into()),
        }
    }

    fn refused(tokens: &[&str]) -> Held {
        Held {
            current: None,
            refused: tokens.iter().map(|t| (*t).to_owned()).collect(),
        }
    }

    #[test]
    fn a_token_another_process_got_for_this_binary_is_used() {
        let now = answers(&[(NEW, granted(1, "T1"))]);
        assert_eq!(
            next(NEW, &Answers::new(), &now, None, &refused(&["T0"])),
            Next::Use("T1".into())
        );
    }

    #[test]
    fn a_token_this_process_saw_refused_is_not_used_again() {
        let now = answers(&[(NEW, granted(1, "T1"))]);
        assert_eq!(
            next(NEW, &now, &now, Some("T1".into()), &refused(&["T1"])),
            Next::Ask
        );
    }

    #[test]
    fn another_binarys_token_is_never_used() {
        let now = answers(&[(OLD, granted(1, "T1"))]);
        // Not from the record, and not from the token file either.
        assert_eq!(
            next(
                NEW,
                &Answers::new(),
                &now,
                Some("T1".into()),
                &Held::default()
            ),
            Next::Ask
        );
    }

    #[test]
    fn a_fresh_process_starts_from_the_stored_token() {
        assert_eq!(
            next(
                NEW,
                &Answers::new(),
                &Answers::new(),
                Some("T0".into()),
                &Held::default()
            ),
            Next::Use("T0".into())
        );
    }

    /// The token file holds whatever was minted last, for whichever binary.
    /// A process that has had a token refused asks rather than guess.
    #[test]
    fn a_process_refused_a_token_doesnt_guess_from_the_stored_one() {
        assert_eq!(
            next(
                NEW,
                &Answers::new(),
                &Answers::new(),
                Some("T5".into()),
                &refused(&["T0"])
            ),
            Next::Ask
        );
    }

    #[test]
    fn a_refusal_made_during_the_wait_is_shared() {
        let before = answers(&[(NEW, granted(1, "T1"))]);
        let now = answers(&[(NEW, denied(2, "user_dismissed"))]);
        assert_eq!(
            next(NEW, &before, &now, None, &refused(&["T1"])),
            Next::Refused("user_dismissed".into())
        );
    }

    #[test]
    fn a_refusal_from_before_the_wait_is_not() {
        let now = answers(&[(NEW, denied(2, "user_dismissed"))]);
        assert_eq!(next(NEW, &now, &now, None, &refused(&["T1"])), Next::Ask);
    }

    #[test]
    fn another_binarys_refusal_is_not_this_ones() {
        let now = answers(&[(OLD, denied(1, "user_denied"))]);
        assert_eq!(
            next(NEW, &Answers::new(), &now, None, &refused(&["T0"])),
            Next::Ask
        );
    }

    /// Only this user may read the tokens on record.
    #[test]
    fn the_record_is_private() {
        use std::os::unix::fs::PermissionsExt;

        let scratch = Scratch::new("private");
        let process = scratch.process();
        let answers = answers(&[(NEW, granted(1, "T1"))]);
        process.write_answers(&answers);

        assert_eq!(process.read_answers(), answers);
        let mode = |path: &Path| fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&scratch.0), 0o700);
        assert_eq!(mode(&scratch.0.join(ANSWERS_FILE)), 0o600);
    }

    #[test]
    fn a_record_that_doesnt_read_is_no_record() {
        let scratch = Scratch::new("garbled");
        make_dir(&scratch.0).unwrap();
        fs::write(scratch.0.join(ANSWERS_FILE), "{not json").unwrap();
        assert_eq!(scratch.process().read_answers(), Answers::new());
    }
}
