/*
 * smarthome-app: the link to the house's data. The dashboard bundle (vendor/sdv-everyware/
 * everyware/bundles/dashboard) subscribes to the house's topics and pushes every update here
 * over gRPC on 127.0.0.1; this process only listens. Every message and every heartbeat
 * becomes an action; a watchdog turns silence into one too. What goes the other way, the
 * screen's Wi-Fi and Matter commands, goes down server streams the bundle opens: the app never
 * opens a connection, and the Wi-Fi and Matter bundles decide what a command means.
 */

use crate::actions::HomeAction;
use futures::channel::mpsc::{unbounded, UnboundedSender};
use futures::{SinkExt, StreamExt};
use grpcio::{Environment, RpcContext, ServerBuilder, ServerCredentials, ServerStreamingSink, UnarySink, WriteFlags};
use harry::reducer::Actions;
use everyware_dashboard_grpc::dashboard::{Ack, HeartbeatRequest, MatterCommandResult, Subscribe, WifiCommandResult};
use everyware_dashboard_grpc::matter::MatterDevices;
use everyware_dashboard_grpc::matter_service::MatterCommand;
use everyware_dashboard_grpc::dashboard_grpc::{create_dashboard_service, DashboardService};
use everyware_dashboard_grpc::news::{NewsFeed, NewsThumbnail};
use everyware_dashboard_grpc::weather::Weather;
use everyware_dashboard_grpc::wifi::{WifiCommand, WifiNetworks, WifiState};
use std::sync::mpsc::{channel, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::Duration;

/// The open command stream to the bundle, if any; a command sent while there is none is
/// dropped (the sheet shows the link's state).
static COMMANDS: OnceLock<Arc<Mutex<Option<UnboundedSender<WifiCommand>>>>> = OnceLock::new();

/// The open Matter command stream to the bundle, likewise.
static MATTER_COMMANDS: OnceLock<Arc<Mutex<Option<UnboundedSender<MatterCommand>>>>> = OnceLock::new();

/// A command down an open stream; dropped (and logged) when there is none.
fn send<T>(stream: &OnceLock<Arc<Mutex<Option<UnboundedSender<T>>>>>, what: &str, command: T) -> bool {
    let Some(commands) = stream.get() else {
        log::warn!("house link: no {what} command stream yet, the command is dropped");
        return false;
    };
    let mut commands = commands.lock().expect("command stream");
    match commands.as_mut() {
        Some(sender) if sender.unbounded_send(command).is_ok() => true,
        Some(_) => {
            *commands = None;
            log::warn!("house link: the {what} command stream is gone");
            false
        }
        None => {
            log::warn!("house link: no {what} command stream, the command is dropped");
            false
        }
    }
}

/// Sends a Wi-Fi command to the Wi-Fi bundle through the bridge.
pub fn send_wifi_command(command: WifiCommand) {
    send(&COMMANDS, "Wi-Fi", command);
}

/// Sends a Matter command to the Matter bundle through the bridge; false: it did not go out.
pub fn send_matter_command(command: MatterCommand) -> bool {
    send(&MATTER_COMMANDS, "Matter", command)
}

/// Where the dashboard bundle connects (bundles/dashboard/src/lib.rs HARRY_ADDRESS).
pub const ADDRESS: &str = "127.0.0.1:7100";
/// Heartbeats come once a second; this much silence means the bundle is gone.
const SILENCE: Duration = Duration::from_secs(5);

#[derive(Clone)]
struct Service {
    actions: Sender<Actions<HomeAction>>,
    beats: Sender<()>,
}

impl DashboardService for Service {
    fn publish_weather(&mut self, ctx: RpcContext<'_>, weather: Weather, sink: UnarySink<Ack>) {
        let _ = self.actions.send(Actions::CustomAction(HomeAction::Weather(weather)));
        let _ = self.beats.send(());
        ctx.spawn(async move {
            let _ = sink.success(Ack::new()).await;
        });
    }

    fn publish_news(&mut self, ctx: RpcContext<'_>, news: NewsFeed, sink: UnarySink<Ack>) {
        let _ = self.actions.send(Actions::CustomAction(HomeAction::News(news)));
        let _ = self.beats.send(());
        ctx.spawn(async move {
            let _ = sink.success(Ack::new()).await;
        });
    }

    fn publish_news_thumbnail(&mut self, ctx: RpcContext<'_>, thumbnail: NewsThumbnail, sink: UnarySink<Ack>) {
        let _ = self.actions.send(Actions::CustomAction(HomeAction::NewsThumbnail(thumbnail)));
        ctx.spawn(async move {
            let _ = sink.success(Ack::new()).await;
        });
    }

    fn publish_wifi_networks(&mut self, ctx: RpcContext<'_>, networks: WifiNetworks, sink: UnarySink<Ack>) {
        let _ = self.actions.send(Actions::CustomAction(HomeAction::WifiNetworks(networks)));
        let _ = self.beats.send(());
        ctx.spawn(async move {
            let _ = sink.success(Ack::new()).await;
        });
    }

    fn publish_wifi_state(&mut self, ctx: RpcContext<'_>, state: WifiState, sink: UnarySink<Ack>) {
        let _ = self.actions.send(Actions::CustomAction(HomeAction::WifiState(state)));
        let _ = self.beats.send(());
        ctx.spawn(async move {
            let _ = sink.success(Ack::new()).await;
        });
    }

    /// The Wi-Fi bundle's answer to a command of the screen (through the bridge).
    fn publish_wifi_result(&mut self, ctx: RpcContext<'_>, result: WifiCommandResult, sink: UnarySink<Ack>) {
        let _ = self.actions.send(Actions::CustomAction(HomeAction::WifiResult(result)));
        ctx.spawn(async move {
            let _ = sink.success(Ack::new()).await;
        });
    }

    /// The bundle asks for the screen's commands: they go down this stream from now on.
    fn wifi_commands(&mut self, ctx: RpcContext<'_>, _request: Subscribe, sink: ServerStreamingSink<WifiCommand>) {
        let (tx, rx) = unbounded();
        *COMMANDS.get_or_init(Default::default).lock().expect("command stream") = Some(tx);
        log::info!("house link: the bundle takes Wi-Fi commands");
        let mut sink = sink;
        ctx.spawn(async move {
            let mut rx = rx.map(|command| Ok((command, WriteFlags::default())));
            let _ = sink.send_all(&mut rx).await;
            let _ = sink.close().await;
        });
    }

    fn publish_matter_devices(&mut self, ctx: RpcContext<'_>, devices: MatterDevices, sink: UnarySink<Ack>) {
        let _ = self.actions.send(Actions::CustomAction(HomeAction::MatterDevices(devices)));
        let _ = self.beats.send(());
        ctx.spawn(async move {
            let _ = sink.success(Ack::new()).await;
        });
    }

    /// The Matter bundle's answer to a command of the screen (through the bridge).
    fn publish_matter_result(&mut self, ctx: RpcContext<'_>, result: MatterCommandResult, sink: UnarySink<Ack>) {
        let _ = self.actions.send(Actions::CustomAction(HomeAction::MatterResult(result)));
        ctx.spawn(async move {
            let _ = sink.success(Ack::new()).await;
        });
    }

    /// The bundle asks for the screen's Matter commands: they go down this stream from now on.
    fn matter_commands(&mut self, ctx: RpcContext<'_>, _request: Subscribe, sink: ServerStreamingSink<MatterCommand>) {
        let (tx, rx) = unbounded();
        *MATTER_COMMANDS.get_or_init(Default::default).lock().expect("command stream") = Some(tx);
        log::info!("house link: the bundle takes Matter commands");
        let mut sink = sink;
        ctx.spawn(async move {
            let mut rx = rx.map(|command| Ok((command, WriteFlags::default())));
            let _ = sink.send_all(&mut rx).await;
            let _ = sink.close().await;
        });
    }

    fn heartbeat(&mut self, ctx: RpcContext<'_>, _request: HeartbeatRequest, sink: UnarySink<Ack>) {
        let _ = self.beats.send(());
        ctx.spawn(async move {
            let _ = sink.success(Ack::new()).await;
        });
    }
}

/// Starts the server and the watchdog; returns when the server could not start.
pub fn serve(actions: Sender<Actions<HomeAction>>) {
    let (beats_tx, beats_rx) = channel();
    let service = Service { actions: actions.clone(), beats: beats_tx };
    let env = Arc::new(Environment::new(1));
    let mut server = match ServerBuilder::new(env)
        .register_service(create_dashboard_service(service))
        .build()
        .and_then(|mut server| server.add_listening_port(ADDRESS, ServerCredentials::insecure()).map(|_| server))
    {
        Ok(server) => server,
        Err(err) => {
            log::error!("house link: cannot listen on {ADDRESS}: {err}");
            return;
        }
    };
    server.start();
    log::info!("house link: listening on {ADDRESS}");

    // The watchdog: linked while heartbeats keep coming.
    let mut linked = false;
    loop {
        let alive = match beats_rx.recv_timeout(SILENCE) {
            Ok(()) => true,
            Err(RecvTimeoutError::Timeout) => false,
            Err(RecvTimeoutError::Disconnected) => return,
        };
        if alive != linked {
            linked = alive;
            log::info!("house link: {}", if linked { "up" } else { "silent" });
            if actions.send(Actions::CustomAction(HomeAction::HouseLink(linked))).is_err() {
                return;
            }
        }
    }
}

/// The server thread.
pub fn spawn(actions: Sender<Actions<HomeAction>>) {
    thread::spawn(move || serve(actions));
}
