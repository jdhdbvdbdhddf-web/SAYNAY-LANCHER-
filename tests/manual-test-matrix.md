# Manual test matrix

Run every row on Windows 10/11, a current Linux distribution, macOS on Apple Silicon and macOS on Intel. Record the result and the date.
Nothing in this list may be marked as passing until it was actually run.

| # | Check | Windows | Linux | macOS arm64 | macOS x64 |
| --- | --- | --- | --- | --- | --- |
| 1 | App starts, Home shows real CPU, GPU, RAM and storage values (or "Unavailable") | | | | |
| 2 | Install a current release as Vanilla; every download passes hash checks | | | | |
| 3 | Install the same version as Fabric | | | | |
| 4 | Java is detected, or a managed Java is downloaded and verified | | | | |
| 5 | Microsoft device-code sign-in works with an approved client ID; sign out clears the keychain entry | | | | |
| 6 | Launch Vanilla; the log streams; the game closes cleanly | | | | |
| 7 | Launch Fabric with the six optimization mods installed from the Mods page | | | | |
| 8 | A mod with no release for the instance's version is refused with a clear reason | | | | |
| 9 | Apply each of the five profiles; options.txt changes; the original backup exists in the backups folder | | | | |
| 10 | Restore defaults brings back the original options.txt | | | | |
| 11 | Force a crash; the crash report path is shown | | | | |
| 12 | Benchmark twice in the same world; charts and percentages match the CSV by hand | | | | |
| 13 | Remove an instance with files; its saves folder appears in backups first | | | | |
| 14 | Resource pack and shader pack import and removal | | | | |
| 15 | Installer installs, launches and uninstalls cleanly (EXE, MSI, AppImage, DEB, DMG) | | | | |
| 16 | Idle launcher resource use on Home is low (record CPU and RAM) | | | | |
| 17 | Add an offline test account; launch a singleplayer world with it; confirm an online-mode server refuses it | | | | |
| 18 | Add a Microsoft account, then a second account; switch between them; remove one and confirm its keychain entry is gone | | | | |
| 19 | Install Forge for a current Minecraft version through the Versions page; launch it | | | | |
| 20 | Install an old release (for example 1.8.9) and a snapshot; both launch | | | | |
| 21 | Disconnect the network; the version list shows the saved copy with a warning; reconnect and Refresh list works | | | | |
| 22 | Kill the network mid-install; Retry install finishes without re-downloading verified files | | | | |
| 23 | Mod install shows a confirmation listing every file; declining installs nothing; Disable and Enable rename the jar | | | | |
| 24 | Change a metadata source in Settings to a bad address; errors are clear; Reset restores the default | | | | |
| 25 | Open the troubleshooting log after a launch and confirm no token appears in it | | | | |
