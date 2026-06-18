//! Seneca Phase 1: boot a swarm of Firecracker microVMs all-or-nothing.
//!
//! A swarm is a JSON file listing members. `up` boots one microVM per member;
//! if any fails, every member is torn down so zero are left running. `down`
//! stops a running swarm; `status` shows what's alive.
//!
//! Firecracker boots straight from a per-VM config file (`--no-api
//! --config-file`), so there are no API calls to make here -- one OS process
//! per microVM is the whole data plane.
//!
//! ponytail: no networking yet -- Phase 1 only needs all-or-nothing boot; add
//! tap devices in the phase that needs egress. No control-plane/scheduler
//! service: state is a file under .seneca/, the "scheduler" is the
//! all-or-nothing loop in `up`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::Duration;

use serde::Deserialize;
use serde_json::json;

const BOOT_ARGS: &str = "console=ttyS0 reboot=k panic=1 pci=off";
// ponytail: fixed liveness heuristic; swap for a readiness probe if boots are slow.
const LIVENESS_WAIT: Duration = Duration::from_millis(1000);
const USAGE: &str = "usage: seneca up <swarm.json> | down <name> | status";

#[derive(Deserialize)]
struct Spec {
    name: String,
    members: Vec<Member>,
}

#[derive(Deserialize)]
struct Member {
    name: String,
    kernel: String,
    rootfs: String,
    #[serde(default = "one")]
    vcpus: u32,
    #[serde(default = "default_mem")]
    mem_mib: u32,
}
fn one() -> u32 {
    1
}
fn default_mem() -> u32 {
    256
}

fn state_dir() -> PathBuf {
    // SENECA_STATE_DIR keeps tests hermetic; defaults to .seneca/ for real use.
    std::env::var("SENECA_STATE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(".seneca"))
}

fn firecracker() -> String {
    std::env::var("FIRECRACKER_BIN").unwrap_or_else(|_| "firecracker".into())
}

fn fc_config(m: &Member) -> serde_json::Value {
    json!({
        "boot-source": { "kernel_image_path": m.kernel, "boot_args": BOOT_ARGS },
        "drives": [{
            "drive_id": "rootfs", "path_on_host": m.rootfs,
            "is_root_device": true, "is_read_only": false
        }],
        "machine-config": { "vcpu_count": m.vcpus, "mem_size_mib": m.mem_mib },
    })
}

fn boot_member(m: &Member, workdir: &Path) -> std::io::Result<Child> {
    let cfg = workdir.join(format!("{}.fc.json", m.name));
    // A Value always serializes; expect() keeps this an io::Result fn.
    fs::write(&cfg, serde_json::to_vec(&fc_config(m)).expect("config serializes"))?;
    let log = fs::File::create(workdir.join(format!("{}.log", m.name)))?;
    let errlog = log.try_clone()?;
    Command::new(firecracker())
        .args(["--no-api", "--config-file"])
        .arg(&cfg)
        .stdout(log)
        .stderr(errlog)
        .spawn()
}

fn kill(child: &mut Child) {
    let _ = child.kill(); // disposable VMs: SIGKILL is fine for teardown
    let _ = child.wait();
}

fn up(spec_path: &str) -> Result<(), String> {
    let spec: Spec = serde_json::from_str(&fs::read_to_string(spec_path).map_err(e)?).map_err(e)?;

    // Validate before booting anything -- cheaper to fail here than mid-launch.
    for m in &spec.members {
        for f in [&m.kernel, &m.rootfs] {
            if !Path::new(f).exists() {
                return Err(format!("missing file for member '{}': {}", m.name, f));
            }
        }
    }

    let workdir = state_dir().join(&spec.name);
    fs::create_dir_all(&workdir).map_err(e)?;

    let mut procs: Vec<(String, Child)> = Vec::new();
    for m in &spec.members {
        match boot_member(m, &workdir) {
            Ok(c) => procs.push((m.name.clone(), c)),
            Err(err) => {
                procs.iter_mut().for_each(|(_, c)| kill(c));
                return Err(format!("failed to spawn '{}': {}", m.name, err));
            }
        }
    }

    // All-or-nothing: a bad config makes firecracker exit fast. If any member
    // died, tear the whole swarm down so nothing is left half-started.
    std::thread::sleep(LIVENESS_WAIT);
    // filter() only hands out shared refs, but try_wait() needs &mut -- use a loop.
    let mut dead: Vec<String> = Vec::new();
    for (n, c) in procs.iter_mut() {
        if matches!(c.try_wait(), Ok(Some(_))) {
            dead.push(n.clone());
        }
    }
    if !dead.is_empty() {
        procs.iter_mut().for_each(|(_, c)| kill(c));
        return Err(format!(
            "swarm '{}' failed all-or-nothing; members died: {}",
            spec.name,
            dead.join(", ")
        ));
    }

    let members: serde_json::Map<String, serde_json::Value> =
        procs.iter().map(|(n, c)| (n.clone(), json!(c.id()))).collect();
    let state = json!({ "name": spec.name, "members": members });
    fs::write(
        state_dir().join(format!("{}.json", spec.name)),
        serde_json::to_vec(&state).map_err(e)?,
    )
    .map_err(e)?;

    let names: Vec<&str> = procs.iter().map(|(n, _)| n.as_str()).collect();
    println!(
        "swarm '{}' up: {} microVMs ({})",
        spec.name,
        procs.len(),
        names.join(", ")
    );
    Ok(())
}

fn down(name: &str) -> Result<(), String> {
    let f = state_dir().join(format!("{}.json", name));
    let state: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(&f).map_err(|_| format!("no such swarm: {}", name))?,
    )
    .map_err(e)?;
    if let Some(members) = state["members"].as_object() {
        for pid in members.values() {
            // std has no kill-by-pid; shelling out to `kill` (SIGTERM) is the lazy native option.
            let _ = Command::new("kill").arg(pid.to_string()).status();
        }
    }
    fs::remove_file(&f).map_err(e)?;
    println!("swarm '{}' down", name);
    Ok(())
}

fn pid_alive(pid: &serde_json::Value) -> bool {
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .status()
        .map_or(false, |s| s.success())
}

fn status() -> Result<(), String> {
    let dir = state_dir();
    if !dir.exists() {
        return Ok(());
    }
    let mut files: Vec<PathBuf> = fs::read_dir(&dir)
        .map_err(e)?
        .filter_map(|x| x.ok().map(|d| d.path()))
        .filter(|p| p.extension().map_or(false, |x| x == "json"))
        .collect();
    files.sort();
    for p in files {
        let s: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&p).map_err(e)?).map_err(e)?;
        if let Some(members) = s["members"].as_object() {
            let alive = members.values().filter(|pid| pid_alive(pid)).count();
            println!(
                "{}: {}/{} alive",
                s["name"].as_str().unwrap_or("?"),
                alive,
                members.len()
            );
        }
    }
    Ok(())
}

fn e<E: std::fmt::Display>(err: E) -> String {
    err.to_string()
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let r = match (args.first().map(String::as_str), args.get(1).map(String::as_str)) {
        (Some("up"), Some(spec)) => up(spec),
        (Some("down"), Some(name)) => down(name),
        (Some("status"), None) => status(),
        _ => {
            eprintln!("{}", USAGE);
            std::process::exit(2);
        }
    };
    if let Err(msg) = r {
        eprintln!("{}", msg);
        std::process::exit(1);
    }
}

// ponytail: one self-check for the non-trivial bit -- all-or-nothing teardown.
// Uses a fake firecracker (a shell script), so it needs no KVM and runs anywhere.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_or_nothing() {
        let tmp = std::env::temp_dir().join(format!("seneca_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        // fake firecracker: exit 1 if the config's rootfs path contains "fail"
        // (simulating a boot failure), otherwise act like a running VM.
        let fc = tmp.join("fake_fc.sh");
        fs::write(
            &fc,
            "#!/bin/sh\ncfg=\"\"\nwhile [ $# -gt 0 ]; do case \"$1\" in --config-file) cfg=\"$2\"; shift;; esac; shift; done\n\
             grep -q 'fail' \"$cfg\" && exit 1\nexec sleep 60\n",
        )
        .unwrap();
        Command::new("chmod").arg("+x").arg(&fc).status().unwrap();

        let kernel = tmp.join("vmlinux");
        fs::write(&kernel, "x").unwrap();
        let ok = tmp.join("ok.ext4");
        fs::write(&ok, "x").unwrap();
        let bad = tmp.join("fail.ext4");
        fs::write(&bad, "x").unwrap();

        std::env::set_var("FIRECRACKER_BIN", &fc);
        std::env::set_var("SENECA_STATE_DIR", tmp.join("state"));

        let kernel_s = kernel.to_str().unwrap();
        let ok_s = ok.to_str().unwrap();
        let write_spec = |file: &str, name: &str, rootfs_b: &str| {
            let p = tmp.join(file);
            fs::write(
                &p,
                serde_json::to_vec(&json!({
                    "name": name,
                    "members": [
                        {"name": "a", "kernel": kernel_s, "rootfs": ok_s},
                        {"name": "b", "kernel": kernel_s, "rootfs": rootfs_b},
                    ]
                }))
                .unwrap(),
            )
            .unwrap();
            p
        };

        // failure: member b fails -> up errors and leaves no state behind.
        let bad_spec = write_spec("bad.json", "broken", bad.to_str().unwrap());
        assert!(up(bad_spec.to_str().unwrap()).is_err());
        assert!(
            !state_dir().join("broken.json").exists(),
            "no state file after a failed all-or-nothing boot"
        );

        // success: both members boot -> state written, then down removes it.
        let ok_spec = write_spec("ok.json", "good", ok.to_str().unwrap());
        up(ok_spec.to_str().unwrap()).unwrap();
        assert!(state_dir().join("good.json").exists());
        down("good").unwrap();
        assert!(!state_dir().join("good.json").exists());

        let _ = fs::remove_dir_all(&tmp);
    }
}
