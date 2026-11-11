# Tests

Automated tests live next to the code and run with `cargo test --manifest-path src-tauri/Cargo.toml`:

| Module | What is checked |
| --- | --- |
| `minecraft/version.rs` | Mojang rule evaluation (last matching rule wins), Maven path building |
| `java/mod.rs` | Java version parsing (`1.8.0_392` is 8, `17.0.9` is 17) and runtime selection rules |
| `performance/mod.rs` | Profile recommendation for sample hardware, RAM never above half of installed memory |
| `mods/mod.rs` | Fabric dependency predicates (`>=21`, arrays, `*`) and unsafe file name rejection |
| `benchmark/mod.rs` | Average FPS, 1% low FPS and frame-time spread on known inputs; CSV parsing |
| `system/update.rs` | Release version comparison |
| `authentication/mod.rs` | Offline account UUID matches Java's `nameUUIDFromBytes` |
| `logger.rs` | Tokens and launch arguments are redacted |
| `minecraft/install.rs` | Forge Maven metadata parsing, version text validation |

The frontend is type checked with `npm run typecheck`.

`manual-test-matrix.md` lists the end-to-end checks that need a real Minecraft account and each operating system.
