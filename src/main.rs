use std::sync::Arc;

use sysinfo::System;
use tokio::{process::Command, sync::Mutex};

struct AliveProcess {
    id: u32,
    name: String,
}

#[derive(Default)]
struct Runner {
    processes: Arc<Mutex<Vec<AliveProcess>>>,
}

impl Runner {
    fn spawn_status_updater(&self) -> tokio::task::JoinHandle<()> {
        let processes = Arc::clone(&self.processes);
        tokio::task::spawn(async move {
            loop {
                let system = System::new_all();
                println!("----");
                for process in processes.lock().await.iter() {
                    let pid = &sysinfo::Pid::from(process.id as usize);
                    let alive = if system.processes().contains_key(pid) {
                        "Alive"
                    } else {
                        "Dead"
                    };
                    println!("{} - {}: {}", process.name, process.id, alive);
                }
                println!("----");
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
            }
        })
    }

    async fn run_and_wait_for_status(&self, binary: impl ToString) -> anyhow::Result<()> {
        let processes = Arc::clone(&self.processes);
        let binary = binary.to_string();
        let mut child = Command::new(&binary).kill_on_drop(true).spawn()?;
        let id = child.id().unwrap();

        let process = AliveProcess {
            id,
            name: binary.to_string(),
        };
        processes.lock().await.push(process);

        child.wait().await?;
        Ok(())
    }

    async fn run_fuzzer(&self) -> anyhow::Result<()> {
        self.run_and_wait_for_status("./never_ending.sh").await
    }

    async fn run_timeouter(&self) -> anyhow::Result<()> {
        self.run_and_wait_for_status("./finishes_fast.sh").await
    }

    async fn run_fuzzers(self) -> anyhow::Result<()> {
        tokio::task::spawn(async move {
            let never_ending_1 = self.run_fuzzer();
            let never_ending_2 = self.run_fuzzer();
            let never_ending_3 = self.run_fuzzer();
            let short_lived = self.run_timeouter();

            tokio::select! {
                _ = never_ending_1 => {
                    println!("never_ending_1 finished");
                }
                _ = never_ending_2 => {
                    println!("never_ending_2 finished");
                }
                _ = never_ending_3 => {
                    println!("never_ending_3 finished");
                }
                _ = short_lived => {
                    println!("short_lived finished");
                }
            }
        })
        .await?;
        Ok(())
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let runner = Runner::default();
    let status_updater = runner.spawn_status_updater();
    runner.run_fuzzers().await?;
    status_updater.await?;

    Ok(())
}
