#[test]
fn ipc_uses_valid_stable_dbus_identifiers() {
    assert_eq!(crate::ipc::BUS_NAME, "io.github.k33wee.ClippyLand");
    assert_eq!(crate::ipc::OBJECT_PATH, "/io/github/k33wee/ClippyLand");
    assert_eq!(crate::ipc::INTERFACE_NAME, "io.github.k33wee.ClippyLand");

    assert!(zbus::names::WellKnownName::try_from(crate::ipc::BUS_NAME).is_ok());
    assert!(zbus::zvariant::ObjectPath::try_from(crate::ipc::OBJECT_PATH).is_ok());
    assert!(zbus::names::InterfaceName::try_from(crate::ipc::INTERFACE_NAME).is_ok());
}
