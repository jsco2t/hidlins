//! Shared fixture paths must match the registry's physical-path contract.
mod common;

#[test]
fn shared_cli_fixture_uses_an_absolute_physical_temp_root() {
    let fixture = common::VaultsToml::new();
    let physical = std::fs::canonicalize(fixture.tempdir.path()).unwrap();
    assert!(fixture.tempdir.path().is_absolute());
    assert_eq!(
        fixture.tempdir.path(),
        physical.as_path(),
        "owned fixture must not retain the host temporary-directory alias"
    );
    assert_eq!(fixture.vaults_toml.parent(), Some(physical.as_path()));
    assert!(!fixture.vaults_toml.exists());
}
