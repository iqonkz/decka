//! Charge of the PC (a laptop) and of connected Bluetooth devices (DATA_MODEL.md 10.6). Devices that do not report a charge to
//! Windows (many mice with their own receiver) are not shown: that is a limit of Windows, not an error.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::model::{BatteryDevice, BatteryInfo, VarValue};
use crate::state::Store;

/// How often the devices are listed (and once at the start).
pub const EVERY: Duration = Duration::from_secs(300);
/// At most this many devices are sent.
pub const MAX_DEVICES: usize = 8;
/// Below this the variable `battery.low` counts a device.
pub const LOW_PCT: u32 = 20;

/// Several nodes of one device carry the same name: the highest charge of them is the device's charge. Result sorted by name.
pub fn dedupe_by_name(found: Vec<(String, u32)>) -> Vec<(String, u32)> {
    let mut best: BTreeMap<String, u32> = BTreeMap::new();
    for (name, pct) in found {
        let name = name.trim().to_string();
        if name.is_empty() || pct > 100 {
            continue;
        }
        let entry = best.entry(name).or_insert(pct);
        *entry = (*entry).max(pct);
    }
    best.into_iter().collect()
}

/// A phone that said nothing for this long is no longer shown (it says its charge once a minute).
pub const PHONE_KEPT: Duration = Duration::from_secs(180);

/// The charge a connected phone reported (`phone_status`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Phone {
    pub name: String,
    pub pct: u32,
    pub charging: bool,
}

/// The section: the PC first (when it has a battery), the phones, then the Bluetooth devices by name, at most 8.
pub fn build_info(pc: Option<(u32, bool)>, phones: Vec<Phone>, bluetooth: Vec<(String, u32)>, now_ms: u64) -> BatteryInfo {
    let mut devices = Vec::new();
    if let Some((pct, charging)) = pc {
        devices.push(BatteryDevice { name: "PC".to_string(), pct: pct.min(100), kind: "pc".to_string(), charging: charging.then_some(true) });
    }
    for p in phones {
        devices.push(BatteryDevice { name: p.name, pct: p.pct.min(100), kind: "phone".to_string(), charging: p.charging.then_some(true) });
    }
    for (name, pct) in dedupe_by_name(bluetooth) {
        devices.push(BatteryDevice { name, pct, kind: "bt".to_string(), charging: None });
    }
    devices.truncate(MAX_DEVICES);
    BatteryInfo { devices, updated_at: now_ms }
}

/// `battery.pc` (the PC's charge, only when it has a battery) and `battery.low` (devices below 20%).
pub fn apply_vars(vars: &mut BTreeMap<String, VarValue>, info: Option<&BatteryInfo>) {
    vars.retain(|k, _| !k.starts_with("battery."));
    let Some(info) = info else { return };
    if let Some(pc) = info.devices.iter().find(|d| d.kind == "pc") {
        vars.insert("battery.pc".to_string(), VarValue::Num(serde_json::Number::from(pc.pct)));
    }
    let low = info.devices.iter().filter(|d| d.pct < LOW_PCT).count();
    vars.insert("battery.low".to_string(), VarValue::Num(serde_json::Number::from(low)));
}

pub fn clear_vars(vars: &mut BTreeMap<String, VarValue>) {
    apply_vars(vars, None);
}

/// What the PC read last: its own battery and the Bluetooth devices.
type PcReading = (Option<(u32, bool)>, Vec<(String, u32)>);

#[derive(Default)]
struct Known {
    /// `None` while the collector does not run.
    pc: Option<PcReading>,
    /// By device id.
    phones: BTreeMap<String, (Phone, Instant)>,
}

/// Puts the section `battery` together from what the PC reads (only while a widget or a variable needs it) and what the
/// phones say about themselves (always, T30.3).
#[derive(Clone)]
pub struct Hub {
    store: Store,
    known: Arc<Mutex<Known>>,
}

impl Hub {
    pub fn new(store: Store) -> Hub {
        Hub { store, known: Arc::default() }
    }

    fn change(&self, f: impl FnOnce(&mut Known) -> bool) {
        // The lock is held while the section is written, so that two changes arrive in their order.
        let mut known = self.known.lock().unwrap();
        if !f(&mut known) {
            return;
        }
        let phones: Vec<Phone> = known.phones.values().map(|(p, _)| p.clone()).collect();
        if known.pc.is_none() && phones.is_empty() {
            self.store.set("battery", serde_json::Value::Null);
            self.store.update_vars(clear_vars);
            return;
        }
        let (pc, bluetooth) = known.pc.clone().unwrap_or_default();
        let info = build_info(pc, phones, bluetooth, crate::auth::now_ms());
        self.store.update_vars(|v| apply_vars(v, Some(&info)));
        self.store.set("battery", serde_json::to_value(&info).expect("battery info serializes"));
    }

    /// A reading of the collector.
    pub fn pc(&self, pc: Option<(u32, bool)>, bluetooth: Vec<(String, u32)>) {
        self.change(|k| {
            k.pc = Some((pc, bluetooth));
            true
        });
    }

    /// The collector stopped: what it read is gone.
    pub fn pc_off(&self) {
        self.change(|k| {
            k.pc = None;
            true
        });
    }

    /// `phone_status` of a paired phone; a repeat of the same charge changes nothing but its age.
    pub fn phone(&self, device: &str, phone: Phone, now: Instant) {
        self.change(|k| {
            let same = k.phones.get(device).is_some_and(|(p, _)| *p == phone);
            k.phones.insert(device.to_string(), (phone, now));
            !same
        });
    }

    /// The phone disconnected.
    pub fn forget(&self, device: &str) {
        self.change(|k| k.phones.remove(device).is_some());
    }

    pub fn expire(&self, now: Instant) {
        self.change(|k| {
            let before = k.phones.len();
            k.phones.retain(|_, (_, at)| now.saturating_duration_since(*at) < PHONE_KEPT);
            k.phones.len() != before
        });
    }
}

/// A running collector thread; dropped to end it.
pub struct BatteryCollector {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl BatteryCollector {
    pub fn start(hub: Hub) -> BatteryCollector {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let thread = std::thread::Builder::new()
            .name("decka-battery-collector".to_string())
            .spawn(move || {
                let mut next = Instant::now();
                while !flag.load(Ordering::Relaxed) {
                    if Instant::now() >= next {
                        hub.pc(read_pc(), read_bluetooth());
                        next = Instant::now() + EVERY;
                    }
                    std::thread::sleep(Duration::from_millis(200));
                }
            })
            .expect("spawn battery collector thread");
        BatteryCollector { stop, thread: Some(thread) }
    }
}

impl Drop for BatteryCollector {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// (percent, charging) of the PC's battery; `None` when there is none (a desktop) or Windows does not know.
#[cfg(windows)]
fn read_pc() -> Option<(u32, bool)> {
    use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
    let mut status = SYSTEM_POWER_STATUS::default();
    // SAFETY: `status` is a valid out structure for the call.
    unsafe { GetSystemPowerStatus(&mut status) }.ok()?;
    // 128 = no system battery; 255 = unknown percentage.
    if status.BatteryFlag & 128 != 0 || status.BatteryLifePercent > 100 {
        return None;
    }
    Some((u32::from(status.BatteryLifePercent), status.BatteryFlag & 8 != 0))
}

#[cfg(not(windows))]
fn read_pc() -> Option<(u32, bool)> {
    None
}

/// Present devices that report a charge through the property `{104EA319-6EE2-4701-BD47-8DDBF425BBE5}, 2` (one byte, 0..100).
#[cfg(windows)]
fn read_bluetooth() -> Vec<(String, u32)> {
    use windows::core::{GUID, PCWSTR};
    use windows::Win32::Devices::DeviceAndDriverInstallation::{
        SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInfo, SetupDiGetClassDevsW, SetupDiGetDevicePropertyW, DIGCF_ALLCLASSES, DIGCF_PRESENT, SP_DEVINFO_DATA,
    };
    use windows::Win32::Devices::Properties::{DEVPKEY_Device_FriendlyName, DEVPROPTYPE};
    use windows::Win32::Foundation::DEVPROPKEY;

    let charge_key = DEVPROPKEY { fmtid: GUID::from_u128(0x104EA319_6EE2_4701_BD47_8DDBF425BBE5), pid: 2 };
    let mut found = Vec::new();
    // SAFETY: the set is created, walked with valid out structures and destroyed below; buffers outlive each call.
    unsafe {
        let Ok(set) = SetupDiGetClassDevsW(None, PCWSTR::null(), None, DIGCF_PRESENT | DIGCF_ALLCLASSES) else { return found };
        let mut index = 0u32;
        loop {
            let mut data = SP_DEVINFO_DATA { cbSize: std::mem::size_of::<SP_DEVINFO_DATA>() as u32, ..Default::default() };
            if SetupDiEnumDeviceInfo(set, index, &mut data).is_err() {
                break;
            }
            index += 1;
            let mut kind = DEVPROPTYPE::default();
            let mut charge = [0u8; 1];
            if SetupDiGetDevicePropertyW(set, &data, &charge_key, &mut kind, Some(&mut charge), None, 0).is_err() {
                continue;
            }
            let mut name_buf = [0u8; 512];
            let mut name_kind = DEVPROPTYPE::default();
            if SetupDiGetDevicePropertyW(set, &data, &DEVPKEY_Device_FriendlyName, &mut name_kind, Some(&mut name_buf), None, 0).is_err() {
                continue;
            }
            let units: Vec<u16> = name_buf.as_chunks::<2>().0.iter().map(|c| u16::from_le_bytes([c[0], c[1]])).take_while(|u| *u != 0).collect();
            found.push((String::from_utf16_lossy(&units), u32::from(charge[0])));
        }
        let _ = SetupDiDestroyDeviceInfoList(set);
    }
    found
}

#[cfg(not(windows))]
fn read_bluetooth() -> Vec<(String, u32)> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found(items: &[(&str, u32)]) -> Vec<(String, u32)> {
        items.iter().map(|(n, p)| (n.to_string(), *p)).collect()
    }

    #[test]
    fn nodes_of_one_device_give_the_highest_charge() {
        let out = dedupe_by_name(found(&[("Headset", 40), ("Mouse", 90), ("Headset", 55), ("Headset", 10)]));
        assert_eq!(out, found(&[("Headset", 55), ("Mouse", 90)]));
    }

    #[test]
    fn empty_names_and_impossible_charges_are_dropped() {
        assert_eq!(dedupe_by_name(found(&[("", 50), ("  ", 20), ("Pad", 101), ("Pad", 70)])), found(&[("Pad", 70)]));
    }

    #[test]
    fn the_pc_comes_first_and_the_list_is_cut_at_eight() {
        let many: Vec<(String, u32)> = (0..12).map(|i| (format!("Dev {i:02}"), 50)).collect();
        let info = build_info(Some((80, true)), Vec::new(), many, 1);
        assert_eq!(info.devices.len(), MAX_DEVICES);
        assert_eq!((info.devices[0].kind.as_str(), info.devices[0].pct, info.devices[0].charging), ("pc", 80, Some(true)));
        assert_eq!(info.devices[1].name, "Dev 00");
        assert!(build_info(None, Vec::new(), Vec::new(), 1).devices.is_empty());
    }

    /// T30.3: the phone is in the section with or without the collector of the PC, and leaves when it is silent or gone.
    #[test]
    fn a_phone_is_shown_without_the_collector_and_leaves_with_its_connection() {
        let store = Store::new(serde_json::json!({}));
        let hub = Hub::new(store.clone());
        let now = Instant::now();
        let phone = |pct, charging| Phone { name: "Pixel 7".into(), pct, charging };
        let devices = || store.get("battery").get("devices").cloned();
        hub.phone("d1", phone(15, false), now);
        assert_eq!(devices(), Some(serde_json::json!([{ "name": "Pixel 7", "pct": 15, "kind": "phone" }])));
        assert_eq!(store.get("vars")["battery.low"], 1);
        let seen = store.get("battery")["updatedAt"].clone();
        std::thread::sleep(Duration::from_millis(5));
        hub.phone("d1", phone(15, false), now + Duration::from_secs(60));
        assert_eq!(store.get("battery")["updatedAt"], seen, "a repeat changes nothing");
        hub.pc(Some((50, false)), found(&[("Mouse", 40)]));
        hub.phone("d1", phone(16, true), now + Duration::from_secs(120));
        let kinds: Vec<String> = devices().unwrap().as_array().unwrap().iter().map(|d| d["kind"].as_str().unwrap().to_string()).collect();
        assert_eq!(kinds, ["pc", "phone", "bt"]);
        hub.expire(now + Duration::from_secs(120) + PHONE_KEPT);
        assert_eq!(devices().unwrap().as_array().unwrap().len(), 2);
        hub.phone("d1", phone(16, true), now);
        hub.forget("d1");
        hub.pc_off();
        assert_eq!(store.get("battery"), serde_json::Value::Null);
        assert!(store.get("vars").get("battery.low").is_none());
    }

    #[test]
    fn variables_count_the_low_devices_and_follow_the_section() {
        let info = build_info(Some((15, false)), Vec::new(), found(&[("Pad", 19), ("Mouse", 20), ("Keys", 80)]), 1);
        let mut vars = BTreeMap::new();
        vars.insert("other".to_string(), VarValue::Bool(true));
        apply_vars(&mut vars, Some(&info));
        assert_eq!(vars.get("battery.pc"), Some(&VarValue::Num(15.into())));
        assert_eq!(vars.get("battery.low"), Some(&VarValue::Num(2.into())));
        clear_vars(&mut vars);
        assert_eq!(vars.keys().collect::<Vec<_>>(), ["other"]);
    }
}
