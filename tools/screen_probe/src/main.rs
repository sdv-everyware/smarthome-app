//! screen_probe: calls the panel's screen link as a stranger and prints what the panel answers.
//!
//!   screen_probe whois <pid> <uid>                  the SDV identity of a process
//!   screen_probe binder                             every IScreenLink call, no identity
//!   screen_probe binder-as <vm> <package> <bundle>  the same after registering that FQIN (root)
//!
//! Every call carries a message that does not parse, so a call the caller check lets through
//! fails with EX_ILLEGAL_ARGUMENT and changes nothing on the screen; EX_SECURITY means the check
//! refused it. The panel shows no news: publishNews and publishNewsThumbnail take a call the check
//! lets through without reading it, so for them ACCEPTED means the check let the caller in.
//! subscribeCommands is the last call: EX_ILLEGAL_STATE means the check let the caller in (the
//! real sink is subscribed). There ACCEPTED happens only when no live sink was there: the probe's
//! sink then dies with the probe, and the screen shows that the house's data is gone until the
//! dashboard bundle subscribes again.

use binder::{BinderFeatures, Interface, Strong};
use com_everyware_screen::aidl::com::everyware::screen::ICommandSink::{BnCommandSink, ICommandSink};
use com_everyware_screen::aidl::com::everyware::screen::IScreenLink::IScreenLink;
use google_sdv_identity::aidl::google::sdv::identity::IIdentityAgent::IIdentityAgent;
use google_sdv_identity::aidl::google::sdv::identity::ProcessIdentifier::ProcessIdentifier;
use google_sdv_identity::aidl::google::sdv::identity::ServiceFqin::ServiceFqin;
use google_sdv_identity::aidl::google::sdv::identity::ServiceIdentity::EcPublicKey::EcPublicKey;

const SCREEN: &str = "com.everyware.screen.IScreenLink/default";
const AGENT: &str = "google.sdv.identity.IIdentityAgent/default";
/// No protobuf message parses from this: a field tag of 0.
const GARBAGE: &[u8] = &[0x00, 0xff, 0xff];

struct Sink;
impl Interface for Sink {}
impl ICommandSink for Sink {
    fn onWifiCommand(&self, _command: &[u8]) -> binder::Result<()> {
        println!("a Wi-Fi command reached the probe's sink");
        Ok(())
    }
    fn onMatterCommand(&self, _command: &[u8]) -> binder::Result<()> {
        println!("a Matter command reached the probe's sink");
        Ok(())
    }
    fn onSettingsChange(&self, _request: &[u8]) -> binder::Result<()> {
        println!("a settings change reached the probe's sink");
        Ok(())
    }
    fn onCalendarCommand(&self, _command: &[u8]) -> binder::Result<()> {
        println!("a calendar command reached the probe's sink");
        Ok(())
    }
}

fn agent() -> Strong<dyn IIdentityAgent> {
    binder::check_interface::<dyn IIdentityAgent>(AGENT).expect("identity agent")
}

fn show(call: &str, result: binder::Result<()>) {
    match result {
        Ok(()) => println!("{call}: ACCEPTED"),
        Err(status) => println!("{call}: {:?} {}", status.exception_code(), status),
    }
}

fn calls() {
    let screen = binder::check_interface::<dyn IScreenLink>(SCREEN).expect("screen link");
    show("publishWeather", screen.publishWeather(GARBAGE));
    show("publishNews", screen.publishNews(GARBAGE));
    show("publishNewsThumbnail", screen.publishNewsThumbnail(GARBAGE));
    show("publishWifiNetworks", screen.publishWifiNetworks(GARBAGE));
    show("publishWifiState", screen.publishWifiState(GARBAGE));
    show("publishWifiResult", screen.publishWifiResult("connect", "probe", GARBAGE));
    show("publishMatterDevices", screen.publishMatterDevices(GARBAGE));
    show("publishMatterResult", screen.publishMatterResult("on-off", 1, 1, GARBAGE));
    show("publishAgenda", screen.publishAgenda(GARBAGE));
    show("publishCalendarResult", screen.publishCalendarResult("delete", "probe", GARBAGE));
    let sink = BnCommandSink::new_binder(Sink, BinderFeatures::default());
    show("subscribeCommands", screen.subscribeCommands(&sink));
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("whois") => {
            let process = ProcessIdentifier { pid: args[1].parse().unwrap(), uid: args[2].parse().unwrap() };
            println!("{:?}", agent().findIdentityByProcess(&process));
        }
        Some("binder") => calls(),
        Some("binder-as") => {
            let fqin = ServiceFqin {
                sdvVmName: args[1].clone(),
                sdvPackageName: args[2].clone(),
                serviceBundleName: args[3].clone(),
                serviceInstanceName: "probe".into(),
            };
            let death = binder::binder_impl::Binder::new(()).as_binder();
            match agent().createIdentity(&EcPublicKey { keyValue: [7; 32] }, &fqin, &death) {
                Ok(identity) => println!("registered {:?} as sid {}", identity.name, identity.sid),
                Err(status) => {
                    println!("createIdentity refused: {status}");
                    return;
                }
            }
            calls();
        }
        _ => eprintln!("usage: screen_probe whois <pid> <uid> | binder | binder-as <vm> <package> <bundle>"),
    }
}
