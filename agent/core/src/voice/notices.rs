//! "Notices itself" (T27.8): a short note on the island when something on the PC needs a look - memory, processor, a low battery,
//! a full disk, the end of a focus timer, an AI limit nearly used up. The rules read what the widgets already know (no new
//! sensors), show quietly (no sound), at most once in 30 minutes per rule, and never over a program in full screen.

use serde_json::Value;

/// A rule shows again after this long at the earliest.
pub const COOLDOWN_MS: u64 = 30 * 60_000;
pub const RAM_PCT: f64 = 90.0;
pub const RAM_FOR_MS: u64 = 2 * 60_000;
pub const CPU_PCT: f64 = 90.0;
pub const CPU_FOR_MS: u64 = 2 * 60_000;
pub const BATTERY_PCT: f64 = 15.0;
pub const DISK_FREE_GB: f64 = 5.0;
pub const AI_PCT: f64 = 90.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Rule {
    Ram,
    Cpu,
    Battery,
    Disk,
    Focus,
    Ai,
}

const ALL: [Rule; 6] = [Rule::Ram, Rule::Cpu, Rule::Battery, Rule::Disk, Rule::Focus, Rule::Ai];

/// What the rules look at, one look.
#[derive(Debug, Clone, Default)]
pub struct Sample {
    pub now_ms: u64,
    pub cpu_pct: Option<f64>,
    pub ram_pct: Option<f64>,
    /// The program that uses the most memory, for the memory note.
    pub top_process: Option<String>,
    pub disk_free_gb: Option<f64>,
    /// The lowest charge among the batteries known (name, percent).
    pub battery_low: Option<(String, f64)>,
    pub ai_pct: Option<f64>,
    pub focus_ended: bool,
    pub fullscreen: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    pub rule: Rule,
    pub text: String,
}

#[derive(Debug, Default)]
pub struct Notices {
    over_since: std::collections::HashMap<Rule, u64>,
    shown_at: std::collections::HashMap<Rule, u64>,
}

impl Notices {
    /// The notes that are due now. `enabled` false or a full screen program: none (and nothing is counted as shown).
    pub fn step(&mut self, enabled: bool, s: &Sample) -> Vec<Notice> {
        let mut out = Vec::new();
        for rule in ALL {
            let (on, hold_ms) = match rule {
                Rule::Ram => (s.ram_pct.is_some_and(|v| v > RAM_PCT), RAM_FOR_MS),
                Rule::Cpu => (s.cpu_pct.is_some_and(|v| v > CPU_PCT), CPU_FOR_MS),
                Rule::Battery => (s.battery_low.as_ref().is_some_and(|b| b.1 <= BATTERY_PCT), 0),
                Rule::Disk => (s.disk_free_gb.is_some_and(|v| v < DISK_FREE_GB), 0),
                Rule::Focus => (s.focus_ended, 0),
                Rule::Ai => (s.ai_pct.is_some_and(|v| v >= AI_PCT), 0),
            };
            if !on {
                self.over_since.remove(&rule);
                continue;
            }
            let since = *self.over_since.entry(rule).or_insert(s.now_ms);
            let held = s.now_ms.saturating_sub(since) >= hold_ms;
            let cooled = self.shown_at.get(&rule).is_none_or(|t| s.now_ms.saturating_sub(*t) >= COOLDOWN_MS);
            if enabled && !s.fullscreen && held && cooled {
                self.shown_at.insert(rule, s.now_ms);
                out.push(Notice { rule, text: text(rule, s) });
            }
        }
        out
    }
}

fn text(rule: Rule, s: &Sample) -> String {
    let n = |v: Option<f64>| v.map_or(String::new(), |v| format!("{}", v.round() as i64));
    match rule {
        Rule::Ram => match &s.top_process {
            Some(p) => format!("Memory is {}% full. {p} uses the most", n(s.ram_pct)),
            None => format!("Memory is {}% full", n(s.ram_pct)),
        },
        Rule::Cpu => format!("The processor has been {}% busy for two minutes", n(s.cpu_pct)),
        Rule::Battery => match &s.battery_low {
            Some((name, pct)) => format!("{name}: battery {}%", pct.round() as i64),
            None => "Battery is low".into(),
        },
        Rule::Disk => format!("Less than {} GB left on the disk", n(s.disk_free_gb)),
        Rule::Focus => "The focus timer is over".into(),
        Rule::Ai => format!("An AI limit is {}% used", n(s.ai_pct)),
    }
}

/// A `Sample` from the sections the agent keeps (the same data the `status` answers read).
pub fn sample(now_ms: u64, sys: &Value, battery: &Value, ai: &Value, focus_ended: bool, fullscreen: bool) -> Sample {
    let f = |v: &Value, p: &str| v.pointer(p).and_then(Value::as_f64);
    let ram_pct = match (f(sys, "/mem/usedMb"), f(sys, "/mem/totalMb")) {
        (Some(u), Some(t)) if t > 0.0 => Some(u * 100.0 / t),
        _ => None,
    };
    let disk_free_gb = sys
        .get("disks")
        .and_then(Value::as_array)
        .and_then(|d| d.iter().filter_map(|x| Some((x.get("totalGb")?.as_f64()? - x.get("usedGb")?.as_f64()?).max(0.0))).reduce(f64::min));
    let battery_low = battery
        .get("devices")
        .and_then(Value::as_array)
        .and_then(|d| d.iter().filter_map(|x| Some((x.get("name")?.as_str()?.to_string(), x.get("pct")?.as_f64()?))).min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal)));
    let ai_pct = ["claude", "codex"]
        .iter()
        .filter_map(|k| ai.get(k)?.get("windows")?.as_array())
        .flatten()
        .filter_map(|w| w.get("pct")?.as_f64())
        .reduce(f64::max);
    let top_process = sys.pointer("/top/0/name").and_then(Value::as_str).map(str::to_string);
    Sample { now_ms, cpu_pct: f(sys, "/cpu"), ram_pct, top_process, disk_free_gb, battery_low, ai_pct, focus_ended, fullscreen }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn at(now_ms: u64) -> Sample {
        Sample { now_ms, ..Default::default() }
    }

    #[test]
    fn memory_over_ninety_percent_is_noted_only_after_a_minute() {
        let mut n = Notices::default();
        let hot = |t| Sample { ram_pct: Some(95.0), top_process: Some("chrome.exe".into()), ..at(t) };
        assert!(n.step(true, &hot(0)).is_empty());
        assert!(n.step(true, &hot(119_000)).is_empty());
        let out = n.step(true, &hot(121_000));
        assert_eq!(out.len(), 1);
        assert!(out[0].text.contains("95%") && out[0].text.contains("chrome.exe"), "{}", out[0].text);
        // A dip below the limit starts the minute again.
        let mut n = Notices::default();
        n.step(true, &hot(0));
        n.step(true, &Sample { ram_pct: Some(50.0), ..at(30_000) });
        assert!(n.step(true, &hot(130_000)).is_empty());
    }

    #[test]
    fn the_processor_needs_two_minutes() {
        let mut n = Notices::default();
        let busy = |t| Sample { cpu_pct: Some(97.0), ..at(t) };
        assert!(n.step(true, &busy(0)).is_empty());
        assert!(n.step(true, &busy(119_000)).is_empty());
        assert_eq!(n.step(true, &busy(121_000))[0].rule, Rule::Cpu);
    }

    #[test]
    fn a_rule_shows_at_most_once_in_thirty_minutes() {
        let mut n = Notices::default();
        let low = |t| Sample { disk_free_gb: Some(2.0), ..at(t) };
        assert_eq!(n.step(true, &low(0)).len(), 1);
        assert!(n.step(true, &low(10 * 60_000)).is_empty());
        assert!(n.step(true, &low(29 * 60_000)).is_empty());
        assert_eq!(n.step(true, &low(31 * 60_000)).len(), 1);
    }

    #[test]
    fn nothing_is_shown_when_off_or_over_a_full_screen_program_and_it_waits_for_later() {
        let mut n = Notices::default();
        let s = |fullscreen| Sample { battery_low: Some(("Mouse".into(), 9.0)), fullscreen, ..at(0) };
        assert!(n.step(false, &s(false)).is_empty(), "off");
        assert!(n.step(true, &s(true)).is_empty(), "full screen");
        assert_eq!(n.step(true, &s(false)).len(), 1, "it was not counted as shown, so it comes when the screen is free");
    }

    #[test]
    fn the_other_rules_have_their_limits() {
        let mut n = Notices::default();
        let s = Sample { battery_low: Some(("Pixel".into(), 16.0)), disk_free_gb: Some(5.0), ai_pct: Some(89.0), ..at(0) };
        assert!(n.step(true, &s).is_empty(), "16 % battery, 5 GB and 89 % are all just above the limits");
        let s = Sample { battery_low: Some(("Pixel".into(), 15.0)), disk_free_gb: Some(4.9), ai_pct: Some(90.0), focus_ended: true, ..at(1) };
        let rules: Vec<Rule> = n.step(true, &s).iter().map(|x| x.rule).collect();
        assert_eq!(rules, vec![Rule::Battery, Rule::Disk, Rule::Focus, Rule::Ai]);
    }

    #[test]
    fn the_sample_is_read_from_the_sections() {
        let sys = json!({ "cpu": 40, "mem": { "usedMb": 15000, "totalMb": 16000 }, "disks": [{ "mount": "C:", "usedGb": 400, "totalGb": 500 }, { "mount": "D:", "usedGb": 498, "totalGb": 500 }], "top": [{ "name": "chrome.exe" }] });
        let battery = json!({ "devices": [{ "name": "Laptop", "pct": 80, "kind": "pc" }, { "name": "Mouse", "pct": 12, "kind": "bt" }] });
        let ai = json!({ "claude": { "windows": [{ "label": "5h", "pct": 40 }, { "label": "7d", "pct": 92 }] } });
        let s = sample(5, &sys, &battery, &ai, false, false);
        assert_eq!((s.ram_pct.map(|v| v.round()), s.disk_free_gb, s.ai_pct, s.top_process.as_deref()), (Some(94.0), Some(2.0), Some(92.0), Some("chrome.exe")));
        assert_eq!(s.battery_low, Some(("Mouse".into(), 12.0)));
    }
}
