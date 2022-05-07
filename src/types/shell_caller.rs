use std::{
    process::Command,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

#[derive(Clone)]
pub struct ShellCaller {
    cmd: Arc<Mutex<String>>,
}
impl ShellCaller {
    pub fn new(cmd: String) -> ShellCaller {
        Self {
            cmd: Arc::new(Mutex::new(cmd)),
        }
    }
}
impl super::MsgSender for ShellCaller {
    fn send(&self, msg: &str) {
        let args = shellwords::split(msg).unwrap();
        let cmd = self.cmd.lock().unwrap();
        if let Err(e) = Command::new(cmd.as_str()).args(args).output() {
            eprintln!("WARNING: error executing command `{cmd} {msg}` -> {e}");
        }
        thread::sleep(Duration::from_millis(2)); // give the bar time to process the message before allowing the next
    }
}
