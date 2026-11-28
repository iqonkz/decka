//! T32.H.4: when the agent is killed, its helpers are gone within two seconds. The "agent" here is this test program
//! started again: it starts a helper inside the job object the real helpers join, and is then killed from outside.

#![cfg(windows)]

use std::io::{BufRead, BufReader};
use std::os::windows::io::AsRawHandle;
use std::os::windows::process::CommandExt;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use decka_core::ai::supervisor::Job;

const ROLE: &str = "DECKA_TEST_PARENT";

fn alive(pid: u32) -> bool {
    let mut system = sysinfo::System::new().expect("process list");
    let id = sysinfo::Pid::from_u32(pid);
    system.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[id]), true);
    system.process(id).is_some()
}

#[test]
fn a_killed_agent_leaves_no_helper_behind() {
    if std::env::var_os(ROLE).is_some() {
        // The stand-in for the agent.
        let helper = Command::new("ping.exe").args(["-n", "60", "127.0.0.1"]).stdout(Stdio::null()).creation_flags(0x0800_0000).spawn().unwrap();
        let job = Job::new().unwrap();
        job.assign(helper.as_raw_handle()).unwrap();
        println!("HELPER {}", helper.id());
        std::thread::sleep(Duration::from_secs(60));
        return;
    }
    let mut parent = Command::new(std::env::current_exe().unwrap())
        .args(["a_killed_agent_leaves_no_helper_behind", "--exact", "--nocapture"])
        .env(ROLE, "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let helper: u32 = BufReader::new(parent.stdout.take().unwrap())
        .lines()
        .map_while(Result::ok)
        .find_map(|l| l.split("HELPER ").nth(1).and_then(|n| n.trim().parse().ok()))
        .expect("the stand-in says which helper it started");
    assert!(alive(helper), "the helper runs while its parent lives");
    parent.kill().unwrap();
    parent.wait().unwrap();
    let killed = Instant::now();
    while alive(helper) && killed.elapsed() < Duration::from_secs(2) {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(!alive(helper), "the helper outlived its killed parent");
}
