//! Stable machine identification (spec item 10). No host name, user name or serial number.

use std::process::{Command, Stdio};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Machine {
    pub os: String,
    pub arch: String,
    pub cpu: String,
    pub logical_cpus: usize,
    pub memory_bytes: u64,
}

impl Machine {
    /// Describes the machine this process runs on.
    pub fn detect() -> Machine {
        let (cpu, mem) = if cfg!(target_os = "macos") {
            (
                sysctl("machdep.cpu.brand_string"),
                sysctl("hw.memsize").and_then(|s| s.parse().ok()),
            )
        } else {
            (
                std::fs::read_to_string("/proc/cpuinfo")
                    .ok()
                    .and_then(|t| cpu_model(&t)),
                std::fs::read_to_string("/proc/meminfo")
                    .ok()
                    .and_then(|t| mem_total(&t)),
            )
        };
        Machine {
            os: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
            cpu: cpu.unwrap_or_else(|| "unknown".to_string()),
            logical_cpus: std::thread::available_parallelism().map_or(0, usize::from),
            memory_bytes: mem.unwrap_or(0),
        }
    }

    /// FNV-1a 64-bit of `os|arch|cpu|logical_cpus|memory_bytes`, as 16 hex digits.
    pub fn id(&self) -> String {
        let key = format!(
            "{}|{}|{}|{}|{}",
            self.os, self.arch, self.cpu, self.logical_cpus, self.memory_bytes
        );
        format!("{:016x}", fnv1a64(key.as_bytes()))
    }
}

pub fn fnv1a64(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

fn sysctl(name: &str) -> Option<String> {
    let out = Command::new("sysctl")
        .args(["-n", name])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    let s = String::from_utf8(out.stdout).ok()?.trim().to_string();
    (out.status.success() && !s.is_empty()).then_some(s)
}

/// The first `model name` value in `/proc/cpuinfo` text.
pub fn cpu_model(cpuinfo: &str) -> Option<String> {
    cpuinfo.lines().find_map(|l| {
        let (k, v) = l.split_once(':')?;
        let v = v.trim();
        (k.trim() == "model name" && !v.is_empty()).then(|| v.to_string())
    })
}

/// `MemTotal` from `/proc/meminfo` text, in bytes.
pub fn mem_total(meminfo: &str) -> Option<u64> {
    meminfo.lines().find_map(|l| {
        let rest = l.strip_prefix("MemTotal:")?;
        let kib: u64 = rest.trim().strip_suffix("kB")?.trim().parse().ok()?;
        kib.checked_mul(1024)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fnv_known_answers() {
        // FNV-1a 64 test vectors (Fowler/Noll/Vo reference values).
        assert_eq!(fnv1a64(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a64(b"a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(fnv1a64(b"foobar"), 0x8594_4171_f739_67e8);
    }

    #[test]
    fn id_depends_on_every_field() {
        let m = Machine {
            os: "linux".into(),
            arch: "x86_64".into(),
            cpu: "AMD EPYC".into(),
            logical_cpus: 4,
            memory_bytes: 16 << 30,
        };
        let id = m.id();
        assert_eq!(id.len(), 16);
        assert!(id.bytes().all(|b| b.is_ascii_hexdigit()));
        for changed in [
            Machine {
                os: "macos".into(),
                ..m.clone()
            },
            Machine {
                arch: "aarch64".into(),
                ..m.clone()
            },
            Machine {
                cpu: "AMD EPYC 2".into(),
                ..m.clone()
            },
            Machine {
                logical_cpus: 8,
                ..m.clone()
            },
            Machine {
                memory_bytes: 8 << 30,
                ..m.clone()
            },
        ] {
            assert_ne!(changed.id(), id, "{changed:?}");
        }
        assert_eq!(m.clone().id(), id);
    }

    #[test]
    fn proc_parsers() {
        let cpuinfo = "processor\t: 0\nvendor_id\t: AuthenticAMD\nmodel name\t: AMD EPYC 7763 64-Core Processor\n\nprocessor\t: 1\nmodel name\t: other\n";
        assert_eq!(
            cpu_model(cpuinfo).as_deref(),
            Some("AMD EPYC 7763 64-Core Processor")
        );
        assert_eq!(cpu_model("processor : 0\n"), None);
        assert_eq!(cpu_model("model name\t:\n"), None);
        assert_eq!(
            mem_total("MemTotal:       16384000 kB\nMemFree: 1 kB\n"),
            Some(16_384_000 * 1024)
        );
        assert_eq!(mem_total("MemTotal: lots kB\n"), None);
        assert_eq!(mem_total("MemTotal: 99999999999999999999 kB\n"), None);
        assert_eq!(mem_total(""), None);
    }

    #[test]
    fn detect_has_no_empty_fields() {
        let m = Machine::detect();
        assert!(!m.os.is_empty() && !m.arch.is_empty() && !m.cpu.is_empty());
    }
}
