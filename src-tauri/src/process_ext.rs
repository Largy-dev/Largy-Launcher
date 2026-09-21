//! Stops spawned child processes (java, mostly) from flashing a console
//! window on Windows. A GUI app doesn't have a console of its own, but
//! `Command::spawn` allocates one for the child by default unless told not
//! to — noticeable as a flicker on every game launch, and as a burst of
//! windows during a Forge/NeoForge install (one java process per processor
//! step).

#[cfg(windows)]
pub fn hide_console_window(cmd: &mut tokio::process::Command) {
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    cmd.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
pub fn hide_console_window(_cmd: &mut tokio::process::Command) {}
