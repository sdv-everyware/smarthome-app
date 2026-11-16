//! Whether the zones go on planes of the display: the flag of the command line, which the APEX's
//! init script passes on every board, and the property by which a board without planes for zones
//! turns it off (Cuttlefish's virtio-gpu has no zpos). No device code: its tests run on the host
//! (smarthome_app_host_test).

/// The property by which a board turns `--zone-plane` off, when its display has no planes a zone
/// can go to. Read once at the start (main.rs).
pub const PROPERTY: &str = "ro.vendor.everyware.zone_plane";

/// Zones on planes: asked for on the command line (`flag`) and not turned off by the board
/// (`property`: the property's value, empty when the board does not set it).
pub fn on(flag: bool, property: &str) -> bool {
    flag && property != "false"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_board_turns_the_zone_planes_off_only_with_false() {
        assert!(on(true, ""), "a board that does not set the property keeps the planes (the RB3)");
        assert!(on(true, "true"));
        assert!(!on(true, "false"), "Cuttlefish");
        assert!(!on(false, ""), "no --zone-plane, no planes");
        assert!(!on(false, "true"), "the property only turns planes off, never on");
    }
}
