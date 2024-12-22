use std::{
    path::PathBuf,
    sync::atomic::{AtomicU32, Ordering},
};

use crossbeam_channel::{Receiver, Sender};

use super::GuiRsp;

pub fn start_vlc_thread(rx: Receiver<PathBuf>, tx: Sender<GuiRsp>) {
    std::thread::spawn(move || {
        let pid = std::sync::Arc::<AtomicU32>::new(AtomicU32::new(u32::MAX));
        for next_path in rx.iter() {
            let binary = "autocrop-vid.sh";

            if pid.load(Ordering::SeqCst) != u32::MAX {
                // println!("killing {}", pid.load(Ordering::SeqCst));
                std::thread::spawn(|| {
                    std::process::Command::new("killall")
                        .arg("-9")
                        .arg("vlc")
                        .spawn()
                        .unwrap()
                        .wait()
                });
                pid.store(u32::MAX, Ordering::SeqCst)
            }

            let child = std::process::Command::new(binary)
                .arg(next_path.clone())
                .spawn();

            std::thread::spawn({
                dbg!(&next_path);
                let pid = pid.clone();
                let tx = tx.clone();
                move || {
                    tx.send(GuiRsp::VlcOpened).unwrap();
                    if let Ok(mut child) = child {
                        pid.store(child.id(), Ordering::SeqCst);
                        let _ = child.wait();
                    } else {
                        dbg!("whoops");
                    }

                    tx.send(GuiRsp::VlcClosed).unwrap();
                }
            });
        }
    });
}
