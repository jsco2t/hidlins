# TUI Vault Onboarding and Unlock Modal

When no vaults are defined in the configuration, the TUI must not exit. It
must take over the terminal and show a modal that asks the user to select a
vault or exit.

The onboarding presentation must incorporate this exact ASCII art:

```text
        .--------.
      .'          '.
     /    .----.    \
    |    |      |    |
.---'----'------'----'---.
| o      HIDLINS      o  |
|                        |       Please select a vault: 
|       HH     HH        |   
|       HH     HH        |       [OPEN]
|       HHHHHHHHH        |       [EXIT]
|       HH     HH        |
| o     HH     HH     o  |
'------------------------'
```

The illustrated file-path, Open, and Exit controls are examples rather than a
fixed interaction design. The required experience is a full-screen TUI with an
accessible modal that lets the user select an existing vault or exit.

If the user selects a vault, that vault must be saved in their configuration.

If a vault already exists in settings, the startup modal must instead ask for
the vault password and retain an exit action. The implementation plan must
review the interaction for accessibility before product work begins.

## Plan revision 2 — Multiple-vault form

The shared startup modal has three explicit forms:

1. When no vault is configured, ask the user to select an existing vault.
2. When exactly one vault is configured, ask for that vault's password.
3. When multiple vaults are configured, list them and let the user select which
   vault to open. The same modal then transitions to the password form for the
   selected vault.

From the multiple-vault password form, the user must be able to use a visible
Back action or press Escape to return to the vault list and choose again.
