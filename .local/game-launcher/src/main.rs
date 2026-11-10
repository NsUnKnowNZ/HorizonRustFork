//! "Horizon OneBlock.exe": starts `bin\horizon-client.exe` with the folder this program is in
//! as the game folder, passing its arguments on. The game is a console program; started this
//! way it gets no console window. Its log is in `%APPDATA%\horizon\logs\latest.log`.
#![windows_subsystem = "windows"]
use std::process::Command;

fn main() {
    let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|d| d.to_owned()))
    else {
        return;
    };
    let game = dir.join("bin").join("horizon-client.exe");
    let mut command = Command::new(&game);
    command.args(std::env::args_os().skip(1)).current_dir(&dir);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW
        command.creation_flags(0x0800_0000);
    }
    if let Err(e) = command.spawn() {
        let note = format!("Could not start {}: {e}\r\n", game.display());
        let _ = std::fs::write(dir.join("Horizon OneBlock - could not start.txt"), note);
    }
}
