use glob::glob;
use libc;

pub struct PipeSender {
    pub bar_pipe_glob: String,
}
impl PipeSender {
    pub fn send(&self, text: &str) {
        if let Ok(bars) = glob(&self.bar_pipe_glob[..]) {
            for bar in bars {
                if let Ok(pipe) = bar {
                    if let Some(fname) = pipe.to_str() {
                        // Need libc::open to open FIFO buffers in nonblocking mode.
                        unsafe {
                            let bytes = &text.as_bytes()[0] as *const u8;
                            libc::write(
                                libc::open(
                                    &fname.as_bytes()[0] as *const u8 as *const i8,
                                    libc::O_APPEND | libc::O_NONBLOCK | libc::O_WRONLY,
                                ),
                                bytes as *const libc::c_void,
                                text.len(),
                            );
                        }
                    }
                }
            }
        }
    }
}

