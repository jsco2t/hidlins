//! Physical fixture paths must agree with canonical vault and backup identity.
mod common;

#[test]
fn shared_sync_fixture_and_clone_have_physical_paths_without_changing_bytes() {
    let original = common::sync_env::SyncTestEnv::new("physical");
    let before = std::fs::read(original.vault_path()).unwrap();
    let copy = original.clone_to("copy");
    for fixture in [&original, &copy] {
        let path = fixture.vault_path();
        assert!(path.is_absolute());
        assert_eq!(path, std::fs::canonicalize(path).unwrap().as_path());
        assert_eq!(std::fs::read(path).unwrap(), before);
    }
}
