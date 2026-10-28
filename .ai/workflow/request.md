# Makefile Target Cleanup

Clean up and standardize the Makefile targets using the requirements discussed in this conversation.

## Requirements

- The current Makefile exposes too many targets to be useful. Look for opportunities to condense or group targets where that makes sense.
- Standardize target names on this pattern and document it in a new root `AGENTS.md`:

  ```text
  make something        # Runs all of whatever "something" is.
  make something-part   # Runs only "part" of "something".
  ```

- Target names must be ordered as:

  ```text
  make [major-activity]-[portion]-[subportion-if-needed]
  ```

- For aggregate activities, the unqualified major target runs all applicable portions. For example, `make lint` runs every lint activity and `make lint-rust` runs only the Rust portion.
- Some activities must run in isolation and therefore must not have an aggregate root. Launch targets are the explicit example: retain only `make run-*` targets, do not create `make run`, and name the application launch target `run-app`, not `app-run`.
- For every target removed or renamed, search all tracked first-party files for uses and references and update each reference to the replacement form.
- Do not retain removed names as compatibility aliases; cleanup must actually reduce the public target surface.
- Preserve the behavior and discoverability of the developer and CI workflows while applying the new hierarchy.
