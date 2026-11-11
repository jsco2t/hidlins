use std::process::Command;

#[test]
fn help_distributes_the_embedded_eff_wordlist_attribution_before_terminal_setup() {
    let output = Command::new(env!("CARGO_BIN_EXE_hidlins"))
        .arg("--help")
        .output()
        .expect("run help without a terminal or vault");
    assert!(output.status.success());
    let help = String::from_utf8(output.stdout).expect("UTF-8 help");
    assert!(
        !help.contains('\u{1b}'),
        "help must not enter the terminal UI"
    );
    for required in [
        "EFF Large Wordlist",
        "Electronic Frontier Foundation",
        "Joseph Bonneau",
        "CC-BY-4.0",
        "https://creativecommons.org/licenses/by/4.0/",
        "https://www.eff.org/files/2016/07/18/eff_large_wordlist.txt",
        "unmodified",
    ] {
        assert!(
            help.contains(required),
            "missing asset attribution: {required}"
        );
    }
}
