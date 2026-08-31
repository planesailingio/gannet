use gannet::state::{LinkMode, State, tags_match};

fn install(state: &mut State, tag: &str) -> Vec<String> {
    state.record_install(
        "owner/tool",
        "github",
        "tool",
        LinkMode::Symlink,
        tag,
        &format!("tool-{tag}.tar.gz"),
        false,
        100,
    )
}

#[test]
fn third_install_prunes_oldest() {
    let mut state = State::default();
    assert!(install(&mut state, "v1").is_empty());
    assert!(install(&mut state, "v2").is_empty());
    assert_eq!(install(&mut state, "v3"), vec!["v1".to_string()]);

    let pkg = state.get("owner/tool").unwrap();
    assert_eq!(pkg.current, "v3");
    assert_eq!(pkg.previous().unwrap().tag, "v2");
    assert_eq!(pkg.versions.len(), 2);
}

#[test]
fn reinstalling_current_does_not_prune() {
    let mut state = State::default();
    install(&mut state, "v1");
    install(&mut state, "v2");
    assert!(install(&mut state, "v2").is_empty());
    let pkg = state.get("owner/tool").unwrap();
    assert_eq!(pkg.versions.len(), 2);
    assert_eq!(pkg.previous().unwrap().tag, "v1");
}

#[test]
fn rollback_toggles() {
    let mut state = State::default();
    install(&mut state, "v1");
    install(&mut state, "v2");
    let prev = state
        .get("owner/tool")
        .unwrap()
        .previous()
        .unwrap()
        .tag
        .clone();
    state.switch_current("owner/tool", &prev, false).unwrap();
    assert_eq!(state.get("owner/tool").unwrap().current, "v1");
    assert_eq!(
        state.get("owner/tool").unwrap().previous().unwrap().tag,
        "v2"
    );
    state.switch_current("owner/tool", "v2", false).unwrap();
    assert_eq!(state.get("owner/tool").unwrap().current, "v2");
}

#[test]
fn switch_to_missing_version_fails() {
    let mut state = State::default();
    install(&mut state, "v1");
    assert!(state.switch_current("owner/tool", "v9", false).is_err());
    assert!(state.switch_current("missing/pkg", "v1", false).is_err());
}

#[test]
fn fresh_install_has_no_previous() {
    let mut state = State::default();
    install(&mut state, "v1");
    assert!(state.get("owner/tool").unwrap().previous().is_none());
}

#[test]
fn tag_v_prefix_tolerance() {
    assert!(tags_match("v1.2.3", "1.2.3"));
    assert!(tags_match("1.2.3", "v1.2.3"));
    assert!(tags_match("v1.2.3", "v1.2.3"));
    assert!(!tags_match("v1.2.3", "v1.2.4"));

    let mut state = State::default();
    install(&mut state, "v1.2.3");
    let pkg = state.get("owner/tool").unwrap();
    assert!(pkg.version_on_disk("1.2.3").is_some());
}

#[test]
fn save_and_load_round_trip() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("state.json");
    let mut state = State::default();
    install(&mut state, "v1");
    state.save(&path).unwrap();
    let loaded = State::load(&path).unwrap();
    assert_eq!(loaded.get("owner/tool").unwrap().current, "v1");
}

#[test]
fn missing_state_file_is_empty() {
    let tmp = tempfile::tempdir().unwrap();
    let state = State::load(&tmp.path().join("nope.json")).unwrap();
    assert!(state.packages.is_empty());
}

#[test]
fn corrupt_state_file_is_a_hard_error() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("state.json");
    std::fs::write(&path, "{not json").unwrap();
    assert!(State::load(&path).is_err());
}
