//! Keeps the sidebar's session list in step with the daemon.
//!
//! [`run`] lives on the app's tokio runtime, apart from GPUI's executor: it owns the API
//! client and talks to the window only through channels, the same boundary the engine
//! keeps with the UI.

use crate::daemon_link::{LinkOptions, RequestError, bounded, ensure_daemon_with};
use almahrya_api_types::{CreateSessionRequest, EventDto, SessionDto, SessionKindDto};
use almahrya_client::{Client, ClientError};
use almahrya_local::paths::Paths;
use almahrya_ui_kit::{ConnectionStatus, SessionItem, SessionsUpdate};
use futures_util::StreamExt;
use std::time::Duration;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use tokio::task::JoinHandle;

/// How often the daemon is checked; a failed check restarts it.
const HEALTH_INTERVAL: Duration = Duration::from_secs(1);
/// Pause before trying again after the daemon could not be started or reached.
const RETRY_DELAY: Duration = Duration::from_secs(2);

/// What the window asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiCommand {
    NewChatSession,
}

/// What an event means for the list.
#[derive(Debug, PartialEq, Eq)]
pub enum Applied {
    Changed,
    Unchanged,
    /// Events were missed; the list must be fetched again.
    Reload,
}

/// The sessions the sidebar shows, newest first.
#[derive(Debug, Default)]
pub struct SessionList {
    sessions: Vec<SessionDto>,
}

fn created(session: &SessionDto) -> Option<OffsetDateTime> {
    OffsetDateTime::parse(&session.created_at, &Rfc3339).ok()
}

impl SessionList {
    pub fn replace(&mut self, sessions: Vec<SessionDto>) {
        self.sessions = sessions;
        self.sort();
    }

    /// Adds `session` unless a session with its id is already listed.
    pub fn insert(&mut self, session: SessionDto) -> bool {
        if self.sessions.iter().any(|s| s.id == session.id) {
            return false;
        }
        self.sessions.push(session);
        self.sort();
        true
    }

    pub fn apply(&mut self, event: &EventDto) -> Applied {
        match event {
            EventDto::SessionCreated { session } => {
                if self.insert(session.clone()) {
                    Applied::Changed
                } else {
                    Applied::Unchanged
                }
            }
            EventDto::ReplayGap { .. } => Applied::Reload,
        }
    }

    /// Newest first; the sort is stable, so equal or unreadable timestamps keep their order.
    fn sort(&mut self) {
        self.sessions.sort_by_key(|s| std::cmp::Reverse(created(s)));
    }

    pub fn items(&self) -> Vec<SessionItem> {
        self.sessions
            .iter()
            .map(|s| SessionItem {
                id: s.id.clone(),
                title: s.title.clone(),
            })
            .collect()
    }
}

/// The window went away; stop.
struct Closed;

struct Sync {
    ui: UnboundedSender<SessionsUpdate>,
    options: LinkOptions,
    list: SessionList,
    /// Highest event `seq` applied, so a new event stream resumes after it.
    last_seq: u64,
}

impl Sync {
    fn send(&self, update: SessionsUpdate) -> Result<(), Closed> {
        self.ui.send(update).map_err(|_| Closed)
    }

    fn status(&self, status: ConnectionStatus) -> Result<(), Closed> {
        self.send(SessionsUpdate::Status(status))
    }

    fn publish(&self) -> Result<(), Closed> {
        self.send(SessionsUpdate::Sessions(self.list.items()))
    }

    async fn reload(&mut self, client: &Client) -> Result<(), RequestError> {
        let sessions = bounded(self.options.request_timeout, client.list_sessions()).await?;
        self.list.replace(sessions);
        Ok(())
    }
}

/// Connects (starting the daemon when needed), shows the sessions and keeps them current
/// until the window closes. When the daemon stops answering it is started again and the
/// event stream resumes where it left off.
pub async fn run(
    paths: Paths,
    ui: UnboundedSender<SessionsUpdate>,
    mut commands: UnboundedReceiver<UiCommand>,
) {
    let mut sync = Sync {
        ui,
        options: LinkOptions::default(),
        list: SessionList::default(),
        last_seq: 0,
    };
    let _ = connection_loop(&paths, &mut sync, &mut commands).await;
}

async fn connection_loop(
    paths: &Paths,
    sync: &mut Sync,
    commands: &mut UnboundedReceiver<UiCommand>,
) -> Result<(), Closed> {
    loop {
        let link = match ensure_daemon_with(paths, &sync.options).await {
            Ok(link) => link,
            Err(e) => {
                sync.status(ConnectionStatus::Failed(e.to_string()))?;
                tokio::time::sleep(RETRY_DELAY).await;
                continue;
            }
        };
        let client = link.client().clone();
        if let Err(e) = sync.reload(&client).await {
            sync.status(ConnectionStatus::Failed(e.to_string()))?;
            tokio::time::sleep(RETRY_DELAY).await;
            continue;
        }
        sync.publish()?;
        sync.status(ConnectionStatus::Connected)?;
        follow(&client, sync, commands).await?;
        sync.status(ConnectionStatus::Reconnecting)?;
    }
}

/// Aborts the task when dropped, so helpers never outlive the connection they serve.
struct AbortOnDrop<T>(JoinHandle<T>);

impl<T> Drop for AbortOnDrop<T> {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// Checks the daemon every [`HEALTH_INTERVAL`] on its own task and resolves once a check
/// fails or times out, so a silent daemon never blocks the loop that serves the window.
fn watch_health(
    client: Client,
    limit: Duration,
) -> (AbortOnDrop<()>, tokio::sync::oneshot::Receiver<()>) {
    let (dead_tx, dead_rx) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(async move {
        let mut tick = tokio::time::interval(HEALTH_INTERVAL);
        loop {
            tick.tick().await;
            if bounded(limit, client.health()).await.is_err() {
                let _ = dead_tx.send(());
                return;
            }
        }
    });
    (AbortOnDrop(task), dead_rx)
}

/// Applies events and commands until the daemon stops answering. Requests run on their
/// own tasks with a time limit; the loop only waits for results.
async fn follow(
    client: &Client,
    sync: &mut Sync,
    commands: &mut UnboundedReceiver<UiCommand>,
) -> Result<(), Closed> {
    let limit = sync.options.request_timeout;
    let mut events = Box::pin(client.events(sync.last_seq));
    let (_watcher, mut dead) = watch_health(client.clone(), limit);
    let (created_tx, mut created) = unbounded_channel::<Result<SessionDto, RequestError>>();
    let mut creating: Vec<AbortOnDrop<()>> = Vec::new();
    loop {
        tokio::select! {
            item = events.next() => match item {
                Some(Ok(envelope)) => {
                    sync.last_seq = sync.last_seq.max(envelope.seq);
                    match sync.list.apply(&envelope.event) {
                        Applied::Changed => sync.publish()?,
                        Applied::Unchanged => {}
                        Applied::Reload => {
                            if sync.reload(client).await.is_err() {
                                return Ok(());
                            }
                            sync.publish()?;
                        }
                    }
                }
                Some(Err(ClientError::Unauthorized)) => {
                    sync.status(ConnectionStatus::Failed("the daemon refused the token".into()))?;
                    tokio::time::sleep(RETRY_DELAY).await;
                    return Ok(());
                }
                Some(Err(e)) => eprintln!("event stream: {e}"),
                None => return Ok(()),
            },
            command = commands.recv() => match command {
                None => return Err(Closed),
                Some(UiCommand::NewChatSession) => {
                    let client = client.clone();
                    let reply = created_tx.clone();
                    creating.retain(|task| !task.0.is_finished());
                    creating.push(AbortOnDrop(tokio::spawn(async move {
                        let request = CreateSessionRequest {
                            title: "New chat".to_owned(),
                            kind: SessionKindDto::Chat,
                        };
                        let _ = reply.send(bounded(limit, client.create_session(request)).await);
                    })));
                }
            },
            Some(result) = created.recv() => match result {
                // The event will say the same; whichever comes first is shown.
                Ok(session) => {
                    if sync.list.insert(session) {
                        sync.publish()?;
                    }
                }
                Err(RequestError::TimedOut(_)) => return Ok(()),
                Err(e) => eprintln!("could not create a session: {e}"),
            },
            _ = &mut dead => return Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(id: &str, created_at: &str) -> SessionDto {
        SessionDto {
            id: id.to_owned(),
            title: format!("t{id}"),
            kind: SessionKindDto::Chat,
            created_at: created_at.to_owned(),
        }
    }

    fn ids(list: &SessionList) -> Vec<String> {
        list.items().into_iter().map(|i| i.id).collect()
    }

    #[test]
    fn newest_first_and_no_duplicates() {
        let mut list = SessionList::default();
        list.replace(vec![
            session("a", "2026-11-14T10:00:00Z"),
            session("b", "2026-11-14T10:00:00.5Z"),
        ]);
        assert_eq!(ids(&list), ["b", "a"]);
        let created = EventDto::SessionCreated {
            session: session("c", "2026-11-14T11:00:00Z"),
        };
        assert_eq!(list.apply(&created), Applied::Changed);
        assert_eq!(list.apply(&created), Applied::Unchanged);
        assert_eq!(ids(&list), ["c", "b", "a"]);
    }

    #[tokio::test]
    async fn a_request_that_never_answers_times_out() {
        let limit = Duration::from_millis(50);
        let never = std::future::pending::<Result<(), ClientError>>();
        let result = tokio::time::timeout(Duration::from_secs(2), bounded(limit, never))
            .await
            .expect("bounded request hung");
        assert!(matches!(result, Err(RequestError::TimedOut(l)) if l == limit));
    }

    #[test]
    fn replay_gap_asks_for_a_reload() {
        let mut list = SessionList::default();
        let gap = EventDto::ReplayGap {
            oldest_available: 7,
        };
        assert_eq!(list.apply(&gap), Applied::Reload);
    }
}
