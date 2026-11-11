//! house_probe: calls the panel's house link as a stranger and prints what the panel answers.
//!
//!   house_probe whois <pid> <uid>                  the SDV identity of a process
//!   house_probe binder                             IHouseLink.subscribeWifiCommands, no identity
//!   house_probe binder-as <vm> <package> <bundle>  the same after registering that FQIN (root)
//!   house_probe grpc                               the four Wi-Fi calls on 127.0.0.1:7100
//!
//! subscribeWifiCommands is the call: EX_SECURITY means the caller check refused, EX_ILLEGAL_STATE
//! that it let the caller in (the real sink is subscribed), and nothing on the screen changes.
//! ACCEPTED happens only when no live sink was there: the probe's sink then dies with the probe,
//! and the screen's Wi-Fi commands go nowhere until the panel or the dashboard bundle restarts.

use binder::{BinderFeatures, Interface, Strong};
use com_everyware_house::aidl::com::everyware::house::IHouseLink::IHouseLink;
use com_everyware_house::aidl::com::everyware::house::IWifiCommandSink::{BnWifiCommandSink, IWifiCommandSink};
use everyware_dashboard_grpc::dashboard::{Subscribe, WifiCommandResult};
use everyware_dashboard_grpc::dashboard_grpc::DashboardServiceClient;
use everyware_dashboard_grpc::wifi::{WifiNetworks, WifiState};
use futures::StreamExt;
use google_sdv_identity::aidl::google::sdv::identity::IIdentityAgent::IIdentityAgent;
use google_sdv_identity::aidl::google::sdv::identity::ProcessIdentifier::ProcessIdentifier;
use google_sdv_identity::aidl::google::sdv::identity::ServiceFqin::ServiceFqin;
use google_sdv_identity::aidl::google::sdv::identity::ServiceIdentity::EcPublicKey::EcPublicKey;
use grpcio::{ChannelBuilder, EnvBuilder};
use std::sync::Arc;

const HOUSE: &str = "com.everyware.house.IHouseLink/default";
const AGENT: &str = "google.sdv.identity.IIdentityAgent/default";

struct Sink;
impl Interface for Sink {}
impl IWifiCommandSink for Sink {
    fn onCommand(&self, _command: &[u8]) -> binder::Result<()> {
        println!("a command reached the probe's sink");
        Ok(())
    }
}

fn agent() -> Strong<dyn IIdentityAgent> {
    binder::check_interface::<dyn IIdentityAgent>(AGENT).expect("identity agent")
}

fn subscribe() {
    let house = binder::check_interface::<dyn IHouseLink>(HOUSE).expect("house link");
    let sink = BnWifiCommandSink::new_binder(Sink, BinderFeatures::default());
    match house.subscribeWifiCommands(&sink) {
        Ok(()) => println!("subscribeWifiCommands: ACCEPTED (the probe now takes the commands)"),
        Err(status) => println!("subscribeWifiCommands: {:?} {}", status.exception_code(), status),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("whois") => {
            let process = ProcessIdentifier { pid: args[1].parse().unwrap(), uid: args[2].parse().unwrap() };
            println!("{:?}", agent().findIdentityByProcess(&process));
        }
        Some("binder") => subscribe(),
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
            subscribe();
        }
        Some("grpc") => {
            let channel = ChannelBuilder::new(Arc::new(EnvBuilder::new().build())).connect("127.0.0.1:7100");
            let client = DashboardServiceClient::new(channel);
            println!("PublishWifiNetworks: {:?}", client.publish_wifi_networks(&WifiNetworks::new()).map(|_| "ok"));
            println!("PublishWifiState: {:?}", client.publish_wifi_state(&WifiState::new()).map(|_| "ok"));
            println!("PublishWifiResult: {:?}", client.publish_wifi_result(&WifiCommandResult::new()).map(|_| "ok"));
            match client.wifi_commands(&Subscribe::new()) {
                Ok(mut stream) => println!("WifiCommands: {:?}", futures::executor::block_on(stream.next()).map(|r| r.map(|_| "a command"))),
                Err(err) => println!("WifiCommands: {err}"),
            }
        }
        _ => eprintln!("usage: house_probe whois <pid> <uid> | binder | binder-as <vm> <package> <bundle> | grpc"),
    }
}
