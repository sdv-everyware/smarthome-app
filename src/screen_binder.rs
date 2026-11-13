/*
 * smarthome-app: the link to the house's data, over binder. The dashboard bundle
 * (vendor/sdv-everyware/everyware/bundles/dashboard) subscribes to the house's topics and calls
 * com.everyware.screen.IScreenLink/default (vendor/sdv-everyware/everyware/interfaces/screen): it
 * pushes the weather, the news, the Wi-Fi networks and state, the Matter devices, the device's
 * settings and the answers to the screen's commands, and hands in the sink the screen's commands
 * go to. The commands carry
 * the password typed on the screen, so the server takes a call only from the dashboard bundle:
 * the kernel gives the caller's pid and uid with every call, and the SDV identity agent says
 * which service bundle that process is.
 *
 * Whether the link is up comes from binder too: a death recipient on the bundle's sink says when
 * the bundle's process is gone, and the next subscribe that it is back. No heartbeat.
 */

use crate::actions::HomeAction;
use binder::binder_impl::IBinderInternal;
use binder::{BinderFeatures, DeathRecipient, ExceptionCode, IBinder, Interface, Status, StatusCode, Strong, ThreadState};
use com_everyware_screen::aidl::com::everyware::screen::ICommandSink::ICommandSink;
use com_everyware_screen::aidl::com::everyware::screen::IScreenLink::{BnScreenLink, IScreenLink};
use everyware_catalog::matter::MatterDevices;
use everyware_catalog::matter_service::{MatterCommand, MatterResult};
use everyware_catalog::news::{NewsFeed, NewsThumbnail};
use everyware_catalog::weather::Weather;
use everyware_catalog::wifi::{WifiCommand, WifiNetworks, WifiState};
use everyware_catalog::wifi_service::WifiResult;
use google_sdv_identity::aidl::google::sdv::identity::IIdentityAgent::IIdentityAgent;
use google_sdv_identity::aidl::google::sdv::identity::ProcessIdentifier::ProcessIdentifier;
use harry::reducer::Actions;
use protobuf::Message;
use std::sync::mpsc::Sender;
use user_preferences_api::change_notifier::OnSettingsChangeRequest;
use user_preferences_api::user_preferences_management_service::RequestSettingsChangeRequest;
use std::sync::{Mutex, OnceLock};
use std::thread;

/// The name the panel serves under (the VINTF fragment in apex/ declares it).
const SERVICE: &str = "com.everyware.screen.IScreenLink/default";
/// The SDV identity agent: it knows which service bundle runs in which process.
const IDENTITY_AGENT: &str = "google.sdv.identity.IIdentityAgent/default";
/// The only caller: the dashboard bundle (bundles/dashboard/apex, its service bundles manifest).
const CALLER_PACKAGE: &str = "com.everyware.dashboard";
const CALLER_BUNDLE: &str = "DashboardServiceBundle";

/// The Wi-Fi bundle's answer to a command of the screen (IScreenLink.publishWifiResult).
#[derive(Clone, Debug, PartialEq)]
pub struct WifiCommandResult {
    /// "scan", "connect", "forget", "disconnect".
    pub command: String,
    /// The SSID of a connect or forget.
    pub ssid: String,
    pub result: WifiResult,
}

/// The Matter bundle's answer to a command of the screen (IScreenLink.publishMatterResult).
#[derive(Clone, Debug, PartialEq)]
pub struct MatterCommandResult {
    /// "pair", "on-off", "level", "setpoint", "system-mode", "fan-mode".
    pub command: String,
    pub node_id: u64,
    pub endpoint: u32,
    pub result: MatterResult,
}

/// The sink the screen's commands go to, with the death recipient that watches it.
struct Subscriber {
    sink: Strong<dyn ICommandSink>,
    _death: DeathRecipient,
    /// The process (pid, uid) that handed in the sink, after the identity agent named it the
    /// dashboard bundle: its calls pass without asking the agent again, as long as the sink
    /// lives. Any other caller is asked every time (the bundle subscribes before it publishes,
    /// so only a stranger pays for that). Not the uid alone: uid 0 is shared by many processes.
    /// None once the sink died: a pid is reused, and the next process with it must be asked.
    trusted: Option<(i32, u32)>,
}

static SUBSCRIBER: Mutex<Option<Subscriber>> = Mutex::new(None);

/// Where the link's actions go (set by spawn).
static ACTIONS: OnceLock<Mutex<Sender<Actions<HomeAction>>>> = OnceLock::new();

fn act(action: HomeAction) {
    if let Some(actions) = ACTIONS.get() {
        let _ = actions.lock().expect("actions").send(Actions::CustomAction(action));
    }
}

/// The bundle's process is gone: the screen says so. Called with SUBSCRIBER held, so a new
/// subscribe cannot get in between and be followed by this.
fn bundle_gone() {
    act(HomeAction::HouseLink(false));
}

/// Sends a Wi-Fi command to the dashboard bundle; false: it did not go out (logged).
pub fn send_wifi_command(command: WifiCommand) -> bool {
    send("Wi-Fi", &command, |sink, bytes| sink.onWifiCommand(bytes))
}

/// Sends a Matter command to the dashboard bundle; false: it did not go out (logged).
pub fn send_matter_command(command: MatterCommand) -> bool {
    send("Matter", &command, |sink, bytes| sink.onMatterCommand(bytes))
}

/// Asks for a change of the device's settings; the values come back with publishSettings once
/// the group's owner applied them. False: it did not go out (logged).
pub fn send_settings_change(request: RequestSettingsChangeRequest) -> bool {
    send("settings", &request, |sink, bytes| sink.onSettingsChange(bytes))
}

fn send<M: Message>(what: &str, command: &M, call: impl FnOnce(&Strong<dyn ICommandSink>, &[u8]) -> binder::Result<()>) -> bool {
    let Ok(bytes) = command.write_to_bytes() else {
        log::error!("screen link: cannot encode a {what} command");
        return false;
    };
    let mut subscriber = SUBSCRIBER.lock().expect("subscriber");
    let Some(current) = subscriber.as_ref() else {
        log::warn!("screen link: no command sink, the {what} command is dropped");
        return false;
    };
    match call(&current.sink, &bytes) {
        Ok(()) => true,
        // only a dead sink goes: a full async buffer fails a oneway call too
        // the death notice may come later, or not at all for this sink once it is dropped here
        Err(err) if err.transaction_error() == StatusCode::DEAD_OBJECT => {
            log::warn!("screen link: the command sink is gone");
            *subscriber = None;
            bundle_gone();
            false
        }
        Err(err) => {
            log::warn!("screen link: a {what} command did not go out ({err})");
            false
        }
    }
}

struct ScreenLink;

impl Interface for ScreenLink {}

impl ScreenLink {
    /// The caller is the dashboard bundle, or the call fails with EX_SECURITY; Ok: its pid and
    /// uid.
    fn check_caller(&self) -> binder::Result<(i32, u32)> {
        let (pid, uid) = (ThreadState::get_calling_pid(), ThreadState::get_calling_uid());
        // not held across the agent's call: other callers must not queue behind it
        if SUBSCRIBER.lock().expect("subscriber").as_ref().is_some_and(|s| s.trusted == Some((pid, uid))) {
            return Ok((pid, uid));
        }
        let refuse = |why: String| {
            log::warn!("screen link: refused pid {pid} uid {uid}: {why}");
            Err(Status::new_exception_str(ExceptionCode::SECURITY, Some("not the dashboard bundle")))
        };
        let agent = match binder::check_interface::<dyn IIdentityAgent>(IDENTITY_AGENT) {
            Ok(agent) => agent,
            Err(err) => return refuse(format!("no identity agent ({err})")),
        };
        let process = ProcessIdentifier { pid: pid.into(), uid: uid.into() };
        let identity = match agent.findIdentityByProcess(&process) {
            Ok(Some(identity)) => identity,
            Ok(None) => return refuse("no SDV identity".into()),
            Err(err) => return refuse(format!("identity agent failed ({err})")),
        };
        let fqin = &identity.name;
        if fqin.sdvPackageName != CALLER_PACKAGE || fqin.serviceBundleName != CALLER_BUNDLE {
            return refuse(format!("{}.{}", fqin.sdvPackageName, fqin.serviceBundleName));
        }
        log::info!("screen link: the dashboard bundle is uid {uid} ({}/{})", fqin.serviceBundleName, fqin.serviceInstanceName);
        Ok((pid, uid))
    }

    fn act(&self, action: HomeAction) {
        act(action);
    }
}

/// A message the bundle sent, or EX_ILLEGAL_ARGUMENT.
fn parse<M: Message>(bytes: &[u8], what: &str) -> binder::Result<M> {
    M::parse_from_bytes(bytes).map_err(|err| {
        log::warn!("screen link: a {what} that does not parse: {err}");
        Status::new_exception_str(ExceptionCode::ILLEGAL_ARGUMENT, Some(what))
    })
}

impl IScreenLink for ScreenLink {
    fn publishWeather(&self, weather: &[u8]) -> binder::Result<()> {
        self.check_caller()?;
        self.act(HomeAction::Weather(parse::<Weather>(weather, "Weather")?));
        Ok(())
    }

    fn publishNews(&self, news: &[u8]) -> binder::Result<()> {
        self.check_caller()?;
        self.act(HomeAction::News(parse::<NewsFeed>(news, "NewsFeed")?));
        Ok(())
    }

    fn publishNewsThumbnail(&self, thumbnail: &[u8]) -> binder::Result<()> {
        self.check_caller()?;
        self.act(HomeAction::NewsThumbnail(parse::<NewsThumbnail>(thumbnail, "NewsThumbnail")?));
        Ok(())
    }

    fn publishWifiNetworks(&self, networks: &[u8]) -> binder::Result<()> {
        self.check_caller()?;
        self.act(HomeAction::WifiNetworks(parse::<WifiNetworks>(networks, "WifiNetworks")?));
        Ok(())
    }

    fn publishWifiState(&self, state: &[u8]) -> binder::Result<()> {
        self.check_caller()?;
        self.act(HomeAction::WifiState(parse::<WifiState>(state, "WifiState")?));
        Ok(())
    }

    fn publishWifiResult(&self, command: &str, ssid: &str, result: &[u8]) -> binder::Result<()> {
        self.check_caller()?;
        let result = parse::<WifiResult>(result, "WifiResult")?;
        self.act(HomeAction::WifiResult(WifiCommandResult { command: command.to_string(), ssid: ssid.to_string(), result }));
        Ok(())
    }

    fn publishMatterDevices(&self, devices: &[u8]) -> binder::Result<()> {
        self.check_caller()?;
        self.act(HomeAction::MatterDevices(parse::<MatterDevices>(devices, "MatterDevices")?));
        Ok(())
    }

    fn publishMatterResult(&self, command: &str, node_id: i64, endpoint: i32, result: &[u8]) -> binder::Result<()> {
        self.check_caller()?;
        let result = parse::<MatterResult>(result, "MatterResult")?;
        let message = MatterCommandResult { command: command.to_string(), node_id: node_id as u64, endpoint: endpoint as u32, result };
        self.act(HomeAction::MatterResult(message));
        Ok(())
    }

    fn publishSettings(&self, settings: &[u8]) -> binder::Result<()> {
        self.check_caller()?;
        self.act(HomeAction::Settings(parse::<OnSettingsChangeRequest>(settings, "OnSettingsChangeRequest")?));
        Ok(())
    }

    fn subscribeCommands(&self, sink: &Strong<dyn ICommandSink>) -> binder::Result<()> {
        let caller = self.check_caller()?;
        let mut subscriber = SUBSCRIBER.lock().expect("subscriber");
        if subscriber.as_ref().is_some_and(|current| current.sink.as_binder() == sink.as_binder()) {
            // the bundle asks again (it did not see the first answer): nothing changes
            return Ok(());
        }
        if subscriber.as_ref().is_some_and(|current| current.sink.as_binder().is_binder_alive()) {
            log::warn!("screen link: a second command sink is refused");
            return Err(Status::new_exception_str(ExceptionCode::ILLEGAL_STATE, Some("a sink is subscribed")));
        }
        // On a binder thread when the bundle's process died. Only this sink's death counts: a
        // late one of a sink already replaced says nothing about the link.
        let watched = sink.as_binder();
        // The sink stays (dropping it here would drop this recipient inside its own call): only
        // its trust goes, and the next subscribe replaces the dead sink.
        let mut death = DeathRecipient::new(move || {
            let mut subscriber = SUBSCRIBER.lock().expect("subscriber");
            if let Some(current) = subscriber.as_mut().filter(|s| s.sink.as_binder() == watched) {
                log::warn!("screen link: the dashboard bundle is gone");
                current.trusted = None;
                bundle_gone();
            }
        });
        if sink.as_binder().link_to_death(&mut death).is_err() {
            log::warn!("screen link: the command sink died before it was taken");
            return Err(StatusCode::DEAD_OBJECT.into());
        }
        *subscriber = Some(Subscriber { sink: sink.clone(), _death: death, trusted: Some(caller) });
        log::info!("screen link: the bundle takes the screen's commands");
        self.act(HomeAction::HouseLink(true));
        Ok(())
    }
}

/// Registers the service and serves it on binder's thread pool; returns at once.
pub fn spawn(actions: Sender<Actions<HomeAction>>) {
    thread::spawn(move || {
        let _ = ACTIONS.set(Mutex::new(actions));
        let service = ScreenLink;
        let binder = BnScreenLink::new_binder(service, BinderFeatures::default());
        // the pool first, then the registration (as the Thread HAL's service does)
        binder::ProcessState::start_thread_pool();
        if let Err(err) = binder::add_service(SERVICE, binder.as_binder()) {
            log::error!("screen link: cannot register {SERVICE}: {err}");
            return;
        }
        log::info!("screen link: serving {SERVICE}");
        binder::ProcessState::join_thread_pool();
    });
}
