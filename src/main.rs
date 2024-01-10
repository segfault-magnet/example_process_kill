use std::{ops::Deref, sync::Arc};

use sysinfo::System;
use tokio::{process::Command, sync::Mutex};

#[derive(Debug, Clone)]
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
                let status = processes
                    .lock()
                    .await
                    .clone()
                    .into_iter()
                    .map(|process| {
                        let pid = &sysinfo::Pid::from(process.id as usize);
                        let alive = system.processes().contains_key(pid);
                        (process, alive)
                    })
                    .collect::<Vec<_>>();

                let status_string = status
                    .iter()
                    .map(|(process, alive)| {
                        format!("{}({}) -- alive: {alive}", process.name, process.id)
                    })
                    .collect::<Vec<_>>()
                    .join("\n");

                eprintln!("--\n{}\n", status_string);

                if !status.is_empty() && status.iter().all(|(_, alive)| !alive) {
                    break;
                }

                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
            }
        })
    }

    async fn run_and_wait_for_completion(&self, binary: &str) -> anyhow::Result<()> {
        let mut child = Command::new(binary).kill_on_drop(true).spawn()?;

        let process = AliveProcess {
            id: child.id().unwrap(),
            name: binary.to_string(),
        };

        self.processes.lock().await.push(process);

        child.wait().await?;

        Ok(())
    }

    async fn run_fuzzer(&self) -> anyhow::Result<()> {
        self.run_and_wait_for_completion("./never_ending.sh").await
    }

    async fn run_timeouter(&self) -> anyhow::Result<()> {
        self.run_and_wait_for_completion("./finishes_fast.sh").await
    }

    async fn run_fuzzers(self) -> anyhow::Result<()> {
        tokio::task::spawn(async move {
            let never_ending_1 = self.run_fuzzer();
            let never_ending_2 = self.run_fuzzer();
            let never_ending_3 = self.run_fuzzer();
            let short_lived = self.run_timeouter();

            tokio::select! {
                _ = never_ending_1 => {
                    println!("Should not happen")
                }
                _ = never_ending_2 => {
                    println!("Should not happen")
                }
                _ = never_ending_3 => {
                    println!("Should not happen")
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
