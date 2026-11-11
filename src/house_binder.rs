/*
 * smarthome-app: the Wi-Fi part of the link to the house, over binder. The dashboard bundle
 * (vendor/sdv-everyware/everyware/bundles/dashboard) calls com.everyware.house.IHouseLink/default
 * (vendor/sdv-everyware/everyware/interfaces/house): it pushes the networks, the link's state and
 * the Wi-Fi bundle's answers, and hands in the sink the screen's commands go to. The commands
 * carry the password typed on the screen, so the server takes a call only from the dashboard
 * bundle: the kernel gives the caller's pid and uid with every call, and the SDV identity agent
 * says which service bundle that process is.
 */

use crate::actions::HomeAction;
use binder::{BinderFeatures, ExceptionCode, IBinder, Interface, Status, StatusCode, Strong, ThreadState};
use com_everyware_house::aidl::com::everyware::house::IHouseLink::{BnHouseLink, IHouseLink};
use com_everyware_house::aidl::com::everyware::house::IWifiCommandSink::IWifiCommandSink;
use everyware_dashboard_grpc::dashboard::WifiCommandResult;
use everyware_dashboard_grpc::wifi::{WifiCommand, WifiNetworks, WifiState};
use everyware_dashboard_grpc::wifi_service::WifiResult;
use google_sdv_identity::aidl::google::sdv::identity::IIdentityAgent::IIdentityAgent;
use google_sdv_identity::aidl::google::sdv::identity::ProcessIdentifier::ProcessIdentifier;
use harry::reducer::Actions;
use protobuf::Message;
use std::sync::mpsc::Sender;
use std::sync::Mutex;
use std::thread;

/// The name the panel serves under (the VINTF fragment in apex/ declares it).
const SERVICE: &str = "com.everyware.house.IHouseLink/default";
/// The SDV identity agent: it knows which service bundle runs in which process.
const IDENTITY_AGENT: &str = "google.sdv.identity.IIdentityAgent/default";
/// The only caller: the dashboard bundle (bundles/dashboard/apex, its service bundles manifest).
const CALLER_PACKAGE: &str = "com.everyware.dashboard";
const CALLER_BUNDLE: &str = "DashboardServiceBundle";

/// The sink the screen's Wi-Fi commands go to. A sink whose process died stays until the next
/// command fails on it or a new bundle subscribes (no death recipient: nothing to clean up).
static SUBSCRIBER: Mutex<Option<Strong<dyn IWifiCommandSink>>> = Mutex::new(None);

/// Sends a Wi-Fi command to the dashboard bundle; dropped (and logged) when no bundle listens.
pub fn send_wifi_command(command: WifiCommand) {
    let Ok(bytes) = command.write_to_bytes() else {
        log::error!("house binder: cannot encode a Wi-Fi command");
        return;
    };
    let mut subscriber = SUBSCRIBER.lock().expect("subscriber");
    let Some(sink) = subscriber.as_ref() else {
        log::warn!("house binder: no Wi-Fi command sink, the command is dropped");
        return;
    };
    match sink.onCommand(&bytes) {
        Ok(()) => {}
        // only a dead sink goes: a full async buffer fails a oneway call too, and the bundle
        // subscribes again only when it sees the panel restart
        Err(err) if err.transaction_error() == StatusCode::DEAD_OBJECT => {
            log::warn!("house binder: the Wi-Fi command sink is gone");
            *subscriber = None;
        }
        Err(err) => log::warn!("house binder: a Wi-Fi command did not go out ({err})"),
    }
}

struct HouseLink {
    actions: Mutex<Sender<Actions<HomeAction>>>,
    /// The process (pid, uid) the identity agent named the dashboard bundle, so the agent is
    /// asked once per process. Not the uid alone: uid 0 is shared by many processes, and one that
    /// passed (root may register any FQIN) would let every other root process in.
    allowed: Mutex<Option<(i32, u32)>>,
}

impl Interface for HouseLink {}

impl HouseLink {
    /// The caller is the dashboard bundle, or the call fails with EX_SECURITY.
    fn check_caller(&self) -> binder::Result<()> {
        let (pid, uid) = (ThreadState::get_calling_pid(), ThreadState::get_calling_uid());
        // not held across the agent's call: other callers must not queue behind it
        if *self.allowed.lock().expect("allowed caller") == Some((pid, uid)) {
            return Ok(());
        }
        let refuse = |why: String| {
            log::warn!("house binder: refused pid {pid} uid {uid}: {why}");
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
        log::info!("house binder: the dashboard bundle is uid {uid} ({}/{})", fqin.serviceBundleName, fqin.serviceInstanceName);
        *self.allowed.lock().expect("allowed caller") = Some((pid, uid));
        Ok(())
    }

    fn act(&self, action: HomeAction) {
        let _ = self.actions.lock().expect("actions").send(Actions::CustomAction(action));
    }
}

/// A message the bundle sent, or EX_ILLEGAL_ARGUMENT.
fn parse<M: Message>(bytes: &[u8], what: &str) -> binder::Result<M> {
    M::parse_from_bytes(bytes).map_err(|err| {
        log::warn!("house binder: a {what} that does not parse: {err}");
        Status::new_exception_str(ExceptionCode::ILLEGAL_ARGUMENT, Some(what))
    })
}

impl IHouseLink for HouseLink {
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
        let mut message = WifiCommandResult::new();
        message.command = command.to_string();
        message.ssid = ssid.to_string();
        message.result = Some(parse::<WifiResult>(result, "WifiResult")?).into();
        self.act(HomeAction::WifiResult(message));
        Ok(())
    }

    fn subscribeWifiCommands(&self, sink: &Strong<dyn IWifiCommandSink>) -> binder::Result<()> {
        self.check_caller()?;
        let mut subscriber = SUBSCRIBER.lock().expect("subscriber");
        if subscriber.as_ref().is_some_and(|current| current.as_binder() == sink.as_binder()) {
            // the bundle asks again (it did not see the first answer): nothing changes
            return Ok(());
        }
        if subscriber.as_ref().is_some_and(|current| current.as_binder().ping_binder().is_ok()) {
            log::warn!("house binder: a second Wi-Fi command sink is refused");
            return Err(Status::new_exception_str(ExceptionCode::ILLEGAL_STATE, Some("a sink is subscribed")));
        }
        *subscriber = Some(sink.clone());
        log::info!("house binder: the bundle takes Wi-Fi commands");
        Ok(())
    }
}

/// Registers the service and serves it on binder's thread pool; returns at once.
pub fn spawn(actions: Sender<Actions<HomeAction>>) {
    thread::spawn(move || {
        let service = HouseLink { actions: Mutex::new(actions), allowed: Mutex::new(None) };
        let binder = BnHouseLink::new_binder(service, BinderFeatures::default());
        // the pool first, then the registration (as the Thread HAL's service does)
        binder::ProcessState::start_thread_pool();
        if let Err(err) = binder::add_service(SERVICE, binder.as_binder()) {
            log::error!("house binder: cannot register {SERVICE}: {err}");
            return;
        }
        log::info!("house binder: serving {SERVICE}");
        binder::ProcessState::join_thread_pool();
    });
}
