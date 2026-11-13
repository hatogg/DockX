//! «Керування» on Windows: Wi-Fi/Bluetooth radios, airplane mode, mobile hotspot and screen
//! brightness (worker thread, WinRT/WMI/DDC-CI), plus fast main-thread switches: dark theme,
//! microphone, keep awake, warm screen (gamma ramp), output device, lock.

#![allow(non_snake_case)]

use super::util::*;
use crate::ui::island::Controls;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::time::Duration;
use windows::core::{interface, IUnknown, IUnknown_Vtbl, BSTR, GUID, HRESULT, PCWSTR, VARIANT};
use windows::Devices::Bluetooth::{BluetoothConnectionStatus, BluetoothDevice, BluetoothLEDevice};
use windows::Devices::Enumeration::DeviceInformation;
use windows::Devices::Radios::{Radio, RadioKind, RadioState};
use windows::Networking::Connectivity::NetworkInformation;
use windows::Networking::NetworkOperators::{NetworkOperatorTetheringManager, TetheringCapability, TetheringOperationalState};
use windows::Win32::Devices::Display::*;
use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::*;
use windows::Win32::NetworkManagement::WiFi::WlanOpenHandle;
use windows::Win32::System::Com::*;
use windows::Win32::System::Power::*;
use windows::Win32::System::Registry::*;
use windows::Win32::System::Shutdown::LockWorkStation;
use windows::Win32::System::Wmi::*;
use windows::Win32::UI::ColorSystem::{GetDeviceGammaRamp, SetDeviceGammaRamp};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::*;

// ───────────────────────── worker: radios, hotspot, brightness ─────────────────────────

#[derive(Clone, Debug)]
pub enum Cmd {
    Refresh,
    Wifi(bool),
    Bluetooth(bool),
    Airplane(bool),
    Hotspot(bool),
    Brightness(f32),
}

/// Radio/hotspot/brightness part of `Controls` read by the worker.
#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub wifi: Option<bool>,
    pub ssid: Option<String>,
    pub bt: Option<bool>,
    pub bt_devices: Vec<String>,
    pub airplane: Option<bool>,
    pub hotspot: Option<bool>,
    pub hotspot_clients: u32,
    pub brightness: Option<f32>,
}

/// The page is open: poll every 1.5 s. Closed: the worker sleeps until a command arrives.
pub static ACTIVE: AtomicBool = AtomicBool::new(false);

/// Undocumented but stable radio-management COM API Windows itself uses for airplane mode.
#[interface("db3afbfb-08e6-46c6-aa70-bf9a34c30ab7")]
unsafe trait IRadioManager: IUnknown {
    fn IsRMSupported(&self, state: *mut u32) -> HRESULT;
    fn GetUIRadioInstances(&self, instances: *mut *mut core::ffi::c_void) -> HRESULT;
    fn GetSystemRadioState(&self, enabled: *mut i32, sw: *mut i32, reason: *mut i32) -> HRESULT;
    fn SetSystemRadioState(&self, enabled: i32) -> HRESULT;
    fn Refresh(&self) -> HRESULT;
    fn OnHardwareSliderChange(&self, a: i32, b: i32) -> HRESULT;
}
const CLSID_RADIO_MANAGER: GUID = GUID::from_u128(0x581333f6_28db_41be_bc7a_ff201f12f3f6);

struct Worker {
    radios_ok: bool,
    rm: Option<IRadioManager>,
    wmi: Option<IWbemServices>,
    wlan: HANDLE,
    /// radio states saved when airplane mode had to be emulated (no radio manager)
    saved_radios: Vec<(RadioKind, bool)>,
}

pub fn spawn(rx: Receiver<Cmd>, tx: Sender<Snapshot>, wake: impl Fn() + Send + 'static) {
    std::thread::Builder::new()
        .name("DockX controls".into())
        .spawn(move || unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let radios_ok = Radio::RequestAccessAsync().and_then(|op| op.get()).is_ok();
            let rm: Option<IRadioManager> = CoCreateInstance(&CLSID_RADIO_MANAGER, None, CLSCTX_LOCAL_SERVER).ok();
            let mut wlan = HANDLE::default();
            let mut ver = 0u32;
            if WlanOpenHandle(2, None, &mut ver, &mut wlan) != 0 {
                wlan = HANDLE::default();
            }
            let mut w = Worker { radios_ok, rm, wmi: wmi_connect(), wlan, saved_radios: Vec::new() };
            let mut first = true;
            loop {
                let msg = if first {
                    first = false;
                    Ok(Cmd::Refresh)
                } else if ACTIVE.load(Ordering::Relaxed) {
                    rx.recv_timeout(Duration::from_millis(1500))
                } else {
                    rx.recv().map_err(|_| RecvTimeoutError::Disconnected)
                };
                let mut cmds = match msg {
                    Ok(c) => vec![c],
                    Err(RecvTimeoutError::Timeout) => vec![Cmd::Refresh],
                    Err(RecvTimeoutError::Disconnected) => return,
                };
                while let Ok(c) = rx.try_recv() {
                    cmds.push(c);
                }
                // a dragged slider sends many values: only the newest one matters
                let last_b = cmds.iter().rposition(|c| matches!(c, Cmd::Brightness(_)));
                for (i, c) in cmds.iter().enumerate() {
                    match c {
                        Cmd::Brightness(_) if Some(i) != last_b => {}
                        c => w.apply(c),
                    }
                }
                if tx.send(w.snapshot()).is_err() {
                    return;
                }
                wake();
            }
        })
        .ok();
}

impl Worker {
    unsafe fn radios(&self) -> Vec<Radio> {
        if !self.radios_ok {
            return Vec::new();
        }
        match Radio::GetRadiosAsync().and_then(|op| op.get()) {
            Ok(v) => v.into_iter().collect(),
            Err(_) => Vec::new(),
        }
    }

    unsafe fn set_kind(&self, kind: RadioKind, on: bool) {
        for r in self.radios() {
            if r.Kind().ok() == Some(kind) {
                let st = if on { RadioState::On } else { RadioState::Off };
                let _ = r.SetStateAsync(st).and_then(|op| op.get());
            }
        }
    }

    unsafe fn apply(&mut self, c: &Cmd) {
        match c {
            Cmd::Refresh => {}
            Cmd::Wifi(on) => self.set_kind(RadioKind::WiFi, *on),
            Cmd::Bluetooth(on) => self.set_kind(RadioKind::Bluetooth, *on),
            Cmd::Airplane(on) => {
                let mut done = false;
                if let Some(rm) = &self.rm {
                    // the API speaks about radios being enabled: airplane mode = disabled
                    done = rm.SetSystemRadioState(if *on { 0 } else { 1 }).is_ok();
                }
                if !done {
                    if *on {
                        self.saved_radios = self.radios().iter().filter_map(|r| Some((r.Kind().ok()?, r.State().ok()? == RadioState::On))).collect();
                        for r in self.radios() {
                            let _ = r.SetStateAsync(RadioState::Off).and_then(|op| op.get());
                        }
                    } else {
                        let saved = std::mem::take(&mut self.saved_radios);
                        for r in self.radios() {
                            let k = r.Kind().ok();
                            let was_on = saved.iter().find(|(sk, _)| Some(*sk) == k).map(|(_, v)| *v).unwrap_or(true);
                            if was_on {
                                let _ = r.SetStateAsync(RadioState::On).and_then(|op| op.get());
                            }
                        }
                    }
                }
            }
            Cmd::Hotspot(on) => {
                if let Some(m) = tethering() {
                    let op = if *on { m.StartTetheringAsync() } else { m.StopTetheringAsync() };
                    if let Err(e) = op.and_then(|o| o.get()) {
                        log_error("hotspot", &e.message().to_string());
                    }
                }
            }
            Cmd::Brightness(v) => {
                let pct = (v.clamp(0.0, 1.0) * 100.0).round() as u8;
                let ok = self.wmi.as_ref().map(|svc| wmi_set_brightness(svc, pct).is_some()).unwrap_or(false);
                if !ok {
                    ddc_set_brightness(pct as u32);
                }
            }
        }
    }

    unsafe fn snapshot(&self) -> Snapshot {
        let mut s = Snapshot::default();
        let radios = self.radios();
        for r in &radios {
            let on = r.State().ok() == Some(RadioState::On);
            match r.Kind() {
                Ok(RadioKind::WiFi) => s.wifi = Some(s.wifi.unwrap_or(false) || on),
                Ok(RadioKind::Bluetooth) => s.bt = Some(s.bt.unwrap_or(false) || on),
                _ => {}
            }
        }
        s.airplane = if let Some(rm) = &self.rm {
            let (mut en, mut sw, mut reason) = (1i32, 0i32, 0i32);
            if rm.GetSystemRadioState(&mut en, &mut sw, &mut reason).is_ok() {
                Some(en == 0)
            } else {
                None
            }
        } else if radios.is_empty() {
            None
        } else {
            Some(radios.iter().all(|r| r.State().ok() != Some(RadioState::On)))
        };
        if !self.wlan.is_invalid() && s.wifi != Some(false) {
            s.ssid = super::sysmon::wifi_ssid(self.wlan);
        }
        if s.bt == Some(true) {
            s.bt_devices = bt_connected();
        }
        if let Some(m) = tethering() {
            match m.TetheringOperationalState() {
                Ok(TetheringOperationalState::On) => {
                    s.hotspot = Some(true);
                    s.hotspot_clients = m.ClientCount().unwrap_or(0);
                }
                Ok(TetheringOperationalState::InTransition) => s.hotspot = Some(true),
                Ok(_) => s.hotspot = Some(false),
                Err(_) => {}
            }
        }
        s.brightness = match &self.wmi {
            Some(svc) => super::sysmon::brightness(svc).map(|b| b as f32 / 100.0),
            None => ddc_get_brightness(),
        };
        s
    }
}

unsafe fn tethering() -> Option<NetworkOperatorTetheringManager> {
    let prof = NetworkInformation::GetInternetConnectionProfile().ok()?;
    if NetworkOperatorTetheringManager::GetTetheringCapabilityFromConnectionProfile(&prof).ok()? != TetheringCapability::Enabled {
        return None;
    }
    NetworkOperatorTetheringManager::CreateFromConnectionProfile(&prof).ok()
}

unsafe fn bt_connected() -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    let sels = [
        BluetoothDevice::GetDeviceSelectorFromConnectionStatus(BluetoothConnectionStatus::Connected),
        BluetoothLEDevice::GetDeviceSelectorFromConnectionStatus(BluetoothConnectionStatus::Connected),
    ];
    for sel in sels.into_iter().flatten() {
        if let Ok(list) = DeviceInformation::FindAllAsyncAqsFilter(&sel).and_then(|op| op.get()) {
            for d in list {
                if let Ok(n) = d.Name() {
                    let n = n.to_string();
                    if !n.is_empty() && !names.contains(&n) {
                        names.push(n);
                    }
                }
            }
        }
    }
    names
}

unsafe fn wmi_connect() -> Option<IWbemServices> {
    let loc: IWbemLocator = CoCreateInstance(&WbemLocator, None, CLSCTX_INPROC_SERVER).ok()?;
    let svc = loc.ConnectServer(&BSTR::from("ROOT\\WMI"), &BSTR::new(), &BSTR::new(), &BSTR::new(), 0, &BSTR::new(), None).ok()?;
    let _ = CoSetProxyBlanket(&svc, 10, 0, None, RPC_C_AUTHN_LEVEL_CALL, RPC_C_IMP_LEVEL_IMPERSONATE, None, EOAC_NONE);
    super::sysmon::brightness(&svc)?;
    Some(svc)
}

/// Laptop panel: WmiMonitorBrightnessMethods.WmiSetBrightness(Timeout, Brightness).
unsafe fn wmi_set_brightness(svc: &IWbemServices, pct: u8) -> Option<()> {
    let mut class = None;
    svc.GetObject(&BSTR::from("WmiMonitorBrightnessMethods"), WBEM_FLAG_RETURN_WBEM_COMPLETE, None, Some(&mut class), None).ok()?;
    let class = class?;
    let mut in_sig = None;
    class.GetMethod(&h("WmiSetBrightness"), 0, &mut in_sig, std::ptr::null_mut()).ok()?;
    let params = in_sig?.SpawnInstance(0).ok()?;
    let timeout = VARIANT::from(0i32);
    let level = VARIANT::from(pct);
    params.Put(&h("Timeout"), 0, &timeout, 0).ok()?;
    params.Put(&h("Brightness"), 0, &level, 0).ok()?;
    let en = svc
        .ExecQuery(&BSTR::from("WQL"), &BSTR::from("SELECT __PATH FROM WmiMonitorBrightnessMethods"), WBEM_FLAG_FORWARD_ONLY | WBEM_FLAG_RETURN_IMMEDIATELY, None)
        .ok()?;
    let mut done = false;
    loop {
        let mut objs = [None];
        let mut n = 0u32;
        let _ = en.Next(2000, &mut objs, &mut n);
        if n == 0 {
            break;
        }
        let Some(obj) = objs[0].take() else { break };
        let mut path = VARIANT::default();
        if obj.Get(&h("__PATH"), 0, &mut path, None, None).is_ok() {
            let p = BSTR::try_from(&path).unwrap_or_default();
            done |= svc.ExecMethod(&p, &BSTR::from("WmiSetBrightness"), WBEM_GENERIC_FLAG_TYPE(0), None, &params, None, None).is_ok();
        }
    }
    done.then_some(())
}

/// External monitors over DDC/CI (first physical monitor of every display).
unsafe fn with_physical_monitors(mut f: impl FnMut(HANDLE)) {
    unsafe extern "system" fn cb(hm: HMONITOR, _: HDC, _: *mut RECT, lp: LPARAM) -> BOOL {
        let list = &mut *(lp.0 as *mut Vec<HMONITOR>);
        list.push(hm);
        TRUE
    }
    let mut mons: Vec<HMONITOR> = Vec::new();
    let _ = EnumDisplayMonitors(None, None, Some(cb), LPARAM(&mut mons as *mut _ as isize));
    for hm in mons {
        let mut n = 0u32;
        if GetNumberOfPhysicalMonitorsFromHMONITOR(hm, &mut n).is_err() || n == 0 {
            continue;
        }
        let mut arr = vec![PHYSICAL_MONITOR::default(); n as usize];
        if GetPhysicalMonitorsFromHMONITOR(hm, &mut arr).is_ok() {
            for pm in &arr {
                f(pm.hPhysicalMonitor);
            }
            let _ = DestroyPhysicalMonitors(&arr);
        }
    }
}

unsafe fn ddc_get_brightness() -> Option<f32> {
    let mut out = None;
    with_physical_monitors(|h| {
        if out.is_none() {
            let (mut lo, mut cur, mut hi) = (0u32, 0u32, 0u32);
            if GetMonitorBrightness(h, &mut lo, &mut cur, &mut hi) != 0 && hi > lo {
                out = Some((cur.saturating_sub(lo)) as f32 / (hi - lo) as f32);
            }
        }
    });
    out
}

unsafe fn ddc_set_brightness(pct: u32) {
    with_physical_monitors(|h| {
        let (mut lo, mut cur, mut hi) = (0u32, 0u32, 0u32);
        if GetMonitorBrightness(h, &mut lo, &mut cur, &mut hi) != 0 && hi > lo {
            let _ = SetMonitorBrightness(h, lo + (hi - lo) * pct / 100);
        }
    });
}

// ───────────────────────── main thread ─────────────────────────

const PERSONALIZE: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize";

fn reg_dword(key: &str, name: &str) -> Option<u32> {
    unsafe {
        let mut v = 0u32;
        let mut size = 4u32;
        let k = wide(key);
        let n = wide(name);
        RegGetValueW(HKEY_CURRENT_USER, PCWSTR(k.as_ptr()), PCWSTR(n.as_ptr()), RRF_RT_REG_DWORD, None, Some(&mut v as *mut u32 as *mut _), Some(&mut size))
            .is_ok()
            .then_some(v)
    }
}

fn set_reg_dword(key: &str, name: &str, v: u32) -> bool {
    unsafe {
        let k = wide(key);
        let n = wide(name);
        RegSetKeyValueW(HKEY_CURRENT_USER, PCWSTR(k.as_ptr()), PCWSTR(n.as_ptr()), REG_DWORD.0, Some(&v as *const u32 as *const _), 4).is_ok()
    }
}

pub fn dark_mode() -> Option<bool> {
    reg_dword(PERSONALIZE, "AppsUseLightTheme").map(|v| v == 0)
}

/// Switches apps + Windows to the dark (or light) theme and tells running programs.
pub fn set_dark_mode(on: bool) {
    let v = if on { 0 } else { 1 };
    set_reg_dword(PERSONALIZE, "AppsUseLightTheme", v);
    set_reg_dword(PERSONALIZE, "SystemUsesLightTheme", v);
    // a broadcast can wait on hung windows: never on the UI thread
    std::thread::spawn(|| unsafe {
        let topic = wide("ImmersiveColorSet");
        let mut res = 0usize;
        let _ = SendMessageTimeoutW(HWND_BROADCAST, WM_SETTINGCHANGE, WPARAM(0), LPARAM(topic.as_ptr() as isize), SMTO_ABORTIFHUNG, 300, Some(&mut res));
    });
}

unsafe fn capture_volume() -> Option<IAudioEndpointVolume> {
    let en: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).ok()?;
    let d = en.GetDefaultAudioEndpoint(eCapture, eCommunications).ok()?;
    d.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None).ok()
}

/// None when there is no microphone.
pub fn mic_muted() -> Option<bool> {
    unsafe { capture_volume().and_then(|ep| ep.GetMute().ok()).map(|b| b.as_bool()) }
}

pub fn set_mic_muted(m: bool) {
    unsafe {
        let en: Option<IMMDeviceEnumerator> = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).ok();
        let Some(en) = en else { return };
        // mute the default device for both roles (they can differ)
        for role in [eCommunications, eConsole] {
            if let Ok(d) = en.GetDefaultAudioEndpoint(eCapture, role) {
                if let Ok(ep) = d.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None) {
                    let _ = ep.SetMute(m, std::ptr::null());
                }
            }
        }
    }
}

pub fn keep_awake(on: bool) {
    unsafe {
        let flags = if on { ES_CONTINUOUS | ES_SYSTEM_REQUIRED | ES_DISPLAY_REQUIRED } else { ES_CONTINUOUS };
        SetThreadExecutionState(flags);
    }
}

pub fn lock() {
    unsafe {
        let _ = LockWorkStation();
    }
}

pub fn open_uri(uri: &str) {
    unsafe {
        let u = wide(uri);
        let verb = wide("open");
        ShellExecuteW(None, PCWSTR(verb.as_ptr()), PCWSTR(u.as_ptr()), None, None, SW_SHOWNORMAL);
    }
}

// ── warm screen («Захист очей»): a gamma ramp with less blue, restored on exit ──

#[repr(C)]
#[derive(Clone)]
struct Ramp([[u16; 256]; 3]);

pub struct Gamma {
    saved: Vec<(Vec<u16>, Ramp)>,
}

unsafe fn display_names() -> Vec<Vec<u16>> {
    unsafe extern "system" fn cb(hm: HMONITOR, _: HDC, _: *mut RECT, lp: LPARAM) -> BOOL {
        let list = &mut *(lp.0 as *mut Vec<Vec<u16>>);
        let mut mi = MONITORINFOEXW::default();
        mi.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
        if GetMonitorInfoW(hm, &mut mi as *mut _ as *mut MONITORINFO).as_bool() {
            let n = mi.szDevice.iter().position(|c| *c == 0).unwrap_or(mi.szDevice.len());
            let mut v = mi.szDevice[..n].to_vec();
            v.push(0);
            list.push(v);
        }
        TRUE
    }
    let mut v: Vec<Vec<u16>> = Vec::new();
    let _ = EnumDisplayMonitors(None, None, Some(cb), LPARAM(&mut v as *mut _ as isize));
    v
}

impl Gamma {
    pub fn new() -> Gamma {
        Gamma { saved: Vec::new() }
    }

    pub fn on(&self) -> bool {
        !self.saved.is_empty()
    }

    /// Warm tint on every display; false if the driver refuses gamma ramps.
    pub fn enable(&mut self) -> bool {
        if self.on() {
            return true;
        }
        unsafe {
            for name in display_names() {
                let dc = CreateDCW(PCWSTR(name.as_ptr()), PCWSTR(name.as_ptr()), PCWSTR::null(), None);
                if dc.is_invalid() {
                    continue;
                }
                let mut orig = Ramp([[0; 256]; 3]);
                if GetDeviceGammaRamp(dc, &mut orig as *mut _ as *mut _).as_bool() {
                    let mut warm = orig.clone();
                    // ~4500 K: keep red, a little less green, clearly less blue
                    for (ch, k) in [(1usize, 0.88f32), (2, 0.72)] {
                        for i in 0..256 {
                            warm.0[ch][i] = (orig.0[ch][i] as f32 * k) as u16;
                        }
                    }
                    if SetDeviceGammaRamp(dc, &warm as *const _ as *const _).as_bool() {
                        self.saved.push((name.clone(), orig));
                    }
                }
                let _ = DeleteDC(dc);
            }
        }
        self.on()
    }

    pub fn disable(&mut self) {
        unsafe {
            for (name, ramp) in self.saved.drain(..) {
                let dc = CreateDCW(PCWSTR(name.as_ptr()), PCWSTR(name.as_ptr()), PCWSTR::null(), None);
                if !dc.is_invalid() {
                    let _ = SetDeviceGammaRamp(dc, &ramp as *const _ as *const _);
                    let _ = DeleteDC(dc);
                }
            }
        }
    }
}

// ── output devices ──

/// Undocumented but long-stable COM interface every audio switcher uses to change the default device.
#[interface("f8679f50-850a-41cf-9c72-430f290290c8")]
unsafe trait IPolicyConfig: IUnknown {
    fn GetMixFormat(&self, a: PCWSTR, b: *mut *mut core::ffi::c_void) -> HRESULT;
    fn GetDeviceFormat(&self, a: PCWSTR, b: i32, c: *mut *mut core::ffi::c_void) -> HRESULT;
    fn ResetDeviceFormat(&self, a: PCWSTR) -> HRESULT;
    fn SetDeviceFormat(&self, a: PCWSTR, b: *mut core::ffi::c_void, c: *mut core::ffi::c_void) -> HRESULT;
    fn GetProcessingPeriod(&self, a: PCWSTR, b: i32, c: *mut i64, d: *mut i64) -> HRESULT;
    fn SetProcessingPeriod(&self, a: PCWSTR, b: *mut i64) -> HRESULT;
    fn GetShareMode(&self, a: PCWSTR, b: *mut core::ffi::c_void) -> HRESULT;
    fn SetShareMode(&self, a: PCWSTR, b: *mut core::ffi::c_void) -> HRESULT;
    fn GetPropertyValue(&self, a: PCWSTR, b: *const core::ffi::c_void, c: *mut core::ffi::c_void) -> HRESULT;
    fn SetPropertyValue(&self, a: PCWSTR, b: *const core::ffi::c_void, c: *mut core::ffi::c_void) -> HRESULT;
    fn SetDefaultEndpoint(&self, id: PCWSTR, role: ERole) -> HRESULT;
    fn SetEndpointVisibility(&self, id: PCWSTR, visible: i32) -> HRESULT;
}
const CLSID_POLICY_CONFIG: GUID = GUID::from_u128(0x870af99c_171d_4f9e_af0d_e63df40c2bc9);

/// (id, name, is default) of active outputs.
pub fn audio_outputs() -> Vec<(String, String, bool)> {
    unsafe {
        let mut out = Vec::new();
        let Ok(en) = CoCreateInstance::<_, IMMDeviceEnumerator>(&MMDeviceEnumerator, None, CLSCTX_ALL) else { return out };
        let def = en.GetDefaultAudioEndpoint(eRender, eConsole).ok().and_then(|d| dev_id(&d));
        let Ok(col) = en.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE) else { return out };
        let n = col.GetCount().unwrap_or(0);
        for i in 0..n {
            let Ok(d) = col.Item(i) else { continue };
            let Some(id) = dev_id(&d) else { continue };
            let name = d
                .OpenPropertyStore(STGM_READ)
                .ok()
                .and_then(|st| st.GetValue(&PKEY_Device_FriendlyName).ok())
                .map(|v| v.to_string())
                .unwrap_or_default();
            let is_def = def.as_deref() == Some(id.as_str());
            out.push((id, if name.is_empty() { "Пристрій".into() } else { name }, is_def));
        }
        out
    }
}

unsafe fn dev_id(d: &IMMDevice) -> Option<String> {
    let p = d.GetId().ok()?;
    let s = p.to_string().ok();
    CoTaskMemFree(Some(p.0 as *const _));
    s
}

pub fn set_default_output(id: &str) -> bool {
    unsafe {
        let Ok(pc) = CoCreateInstance::<_, IPolicyConfig>(&CLSID_POLICY_CONFIG, None, CLSCTX_ALL) else { return false };
        let w = wide(id);
        let mut ok = true;
        for role in [eConsole, eMultimedia, eCommunications] {
            ok &= pc.SetDefaultEndpoint(PCWSTR(w.as_ptr()), role).is_ok();
        }
        ok
    }
}

/// Full `Controls` from the worker snapshot plus the fast local switches.
pub fn merge(snap: &Snapshot, awake: bool, night: bool, audio_out: &str) -> Controls {
    Controls {
        wifi: snap.wifi,
        ssid: snap.ssid.clone(),
        bt: snap.bt,
        bt_devices: snap.bt_devices.clone(),
        airplane: snap.airplane,
        hotspot: snap.hotspot,
        hotspot_clients: snap.hotspot_clients,
        awake,
        dark: dark_mode(),
        mic_muted: mic_muted(),
        night,
        brightness: snap.brightness,
        audio_out: audio_out.to_string(),
    }
}
