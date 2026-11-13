//! System indicators: battery, volume, Caps Lock, keyboard layout, idle time, and a worker for
//! Wi-Fi, screen brightness and the default audio device.

use super::util::*;
use crate::ui::island::Battery;
use std::sync::mpsc::Sender;
use std::time::Duration;
use windows::core::{BSTR, PCWSTR};
use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
use windows::Win32::Foundation::*;
use windows::Win32::Globalization::*;
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::*;
use windows::Win32::NetworkManagement::WiFi::*;
use windows::Win32::System::Com::*;
use windows::Win32::System::Power::*;
use windows::Win32::System::Wmi::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;

pub fn battery() -> Option<Battery> {
    unsafe {
        let mut st = SYSTEM_POWER_STATUS::default();
        if GetSystemPowerStatus(&mut st).is_err() {
            return None;
        }
        if st.BatteryFlag & 128 != 0 || st.BatteryFlag == 255 || st.BatteryLifePercent > 100 {
            return None; // no system battery
        }
        Some(Battery { percent: st.BatteryLifePercent, charging: st.BatteryFlag & 8 != 0, plugged: st.ACLineStatus == 1 })
    }
}

pub fn caps_on() -> bool {
    unsafe { GetKeyState(VK_CAPITAL.0 as i32) & 1 != 0 }
}

/// (layout id, "Українська")
pub fn layout() -> (u32, String) {
    unsafe {
        let fg = GetForegroundWindow();
        let tid = GetWindowThreadProcessId(fg, None);
        let hkl = GetKeyboardLayout(tid);
        let lang = (hkl.0 as usize & 0xffff) as u32;
        let mut buf = [0u16; 80];
        let n = GetLocaleInfoW(lang, LOCALE_SNATIVELANGUAGENAME, Some(&mut buf));
        let mut name = if n > 0 { from_wide(&buf) } else { format!("{:04x}", lang) };
        if let Some(c) = name.chars().next() {
            name = c.to_uppercase().collect::<String>() + &name[c.len_utf8()..];
        }
        (lang, name)
    }
}

pub fn idle_secs() -> f64 {
    unsafe {
        let mut li = LASTINPUTINFO { cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32, dwTime: 0 };
        if GetLastInputInfo(&mut li).as_bool() {
            let now = windows::Win32::System::SystemInformation::GetTickCount();
            now.wrapping_sub(li.dwTime) as f64 / 1000.0
        } else {
            0.0
        }
    }
}

/// Master volume of the default output (main thread, STA).
pub struct Volume {
    ep: Option<IAudioEndpointVolume>,
    dev: String,
}

impl Volume {
    pub fn new() -> Volume {
        let mut v = Volume { ep: None, dev: String::new() };
        v.refresh();
        v
    }
    pub fn refresh(&mut self) {
        unsafe {
            let Ok(en) = CoCreateInstance::<_, IMMDeviceEnumerator>(&MMDeviceEnumerator, None, CLSCTX_ALL) else { return };
            if let Ok(d) = en.GetDefaultAudioEndpoint(eRender, eConsole) {
                let id = d.GetId().ok().map(|p| {
                    let s = p.to_string().unwrap_or_default();
                    CoTaskMemFree(Some(p.0 as *const _));
                    s
                });
                let id = id.unwrap_or_default();
                if id != self.dev || self.ep.is_none() {
                    self.dev = id;
                    self.ep = d.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None).ok();
                }
            }
        }
    }
    pub fn get(&self) -> Option<(f32, bool)> {
        unsafe {
            let ep = self.ep.as_ref()?;
            let v = ep.GetMasterVolumeLevelScalar().ok()?;
            let m = ep.GetMute().map(|b| b.as_bool()).unwrap_or(false);
            Some((v, m))
        }
    }
    pub fn set(&self, v: f32) {
        unsafe {
            if let Some(ep) = &self.ep {
                let _ = ep.SetMute(false, std::ptr::null());
                let _ = ep.SetMasterVolumeLevelScalar(v.clamp(0.0, 1.0), std::ptr::null());
            }
        }
    }
    pub fn toggle_mute(&self) {
        unsafe {
            if let Some(ep) = &self.ep {
                let m = ep.GetMute().map(|b| b.as_bool()).unwrap_or(false);
                let _ = ep.SetMute(!m, std::ptr::null());
            }
        }
    }
}

#[derive(Clone, Debug)]
pub enum SysEvent {
    Wifi(Option<String>),
    Brightness(f32),
    AudioDevice(String),
}

pub fn spawn_monitor(tx: Sender<SysEvent>, wake: impl Fn() + Send + 'static) {
    std::thread::Builder::new()
        .name("DockX sysmon".into())
        .spawn(move || unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let wmi = wmi_connect();
            let mut wlan = HANDLE::default();
            let mut ver = 0u32;
            let wlan_ok = WlanOpenHandle(2, None, &mut ver, &mut wlan) == 0;
            let enumr: Option<IMMDeviceEnumerator> = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).ok();
            let mut last_bright: Option<u32> = None;
            let mut last_wifi: Option<Option<String>> = None;
            let mut last_audio: Option<String> = None;
            let mut tick = 0u64;
            loop {
                if let Some(svc) = &wmi {
                    if let Some(b) = brightness(svc) {
                        if last_bright.is_some() && last_bright != Some(b) {
                            if tx.send(SysEvent::Brightness(b as f32 / 100.0)).is_err() {
                                return;
                            }
                            wake();
                        }
                        last_bright = Some(b);
                    }
                }
                if tick % 3 == 0 && wlan_ok {
                    let w = wifi_ssid(wlan);
                    if let Some(prev) = &last_wifi {
                        if *prev != w {
                            let _ = tx.send(SysEvent::Wifi(w.clone()));
                            wake();
                        }
                    }
                    last_wifi = Some(w);
                }
                if tick % 2 == 0 {
                    if let Some(en) = &enumr {
                        if let Some(name) = audio_name(en) {
                            if let Some(prev) = &last_audio {
                                if *prev != name {
                                    let _ = tx.send(SysEvent::AudioDevice(name.clone()));
                                    wake();
                                }
                            }
                            last_audio = Some(name);
                        }
                    }
                }
                tick += 1;
                std::thread::sleep(Duration::from_millis(700));
            }
        })
        .ok();
}

unsafe fn wmi_connect() -> Option<IWbemServices> {
    let loc: IWbemLocator = CoCreateInstance(&WbemLocator, None, CLSCTX_INPROC_SERVER).ok()?;
    let svc = loc.ConnectServer(&BSTR::from("ROOT\\WMI"), &BSTR::new(), &BSTR::new(), &BSTR::new(), 0, &BSTR::new(), None).ok()?;
    let _ = CoSetProxyBlanket(&svc, 10, 0, None, RPC_C_AUTHN_LEVEL_CALL, RPC_C_IMP_LEVEL_IMPERSONATE, None, EOAC_NONE);
    // desktops with external monitors have no WmiMonitorBrightness: probe once
    brightness(&svc)?;
    Some(svc)
}

unsafe fn brightness(svc: &IWbemServices) -> Option<u32> {
    let en = svc
        .ExecQuery(&BSTR::from("WQL"), &BSTR::from("SELECT CurrentBrightness FROM WmiMonitorBrightness"), WBEM_FLAG_FORWARD_ONLY | WBEM_FLAG_RETURN_IMMEDIATELY, None)
        .ok()?;
    let mut objs = [None];
    let mut n = 0u32;
    let _ = en.Next(2000, &mut objs, &mut n);
    if n == 0 {
        return None;
    }
    let obj = objs[0].take()?;
    let mut v = windows::core::VARIANT::default();
    obj.Get(&h("CurrentBrightness"), 0, &mut v, None, None).ok()?;
    u32::try_from(&v).ok()
}

unsafe fn wifi_ssid(hc: HANDLE) -> Option<String> {
    let mut list: *mut WLAN_INTERFACE_INFO_LIST = std::ptr::null_mut();
    if WlanEnumInterfaces(hc, None, &mut list) != 0 || list.is_null() {
        return None;
    }
    let l = &*list;
    let infos = std::slice::from_raw_parts(l.InterfaceInfo.as_ptr(), l.dwNumberOfItems as usize);
    let mut out = None;
    for info in infos {
        if info.isState != wlan_interface_state_connected {
            continue;
        }
        let mut size = 0u32;
        let mut data: *mut core::ffi::c_void = std::ptr::null_mut();
        if WlanQueryInterface(hc, &info.InterfaceGuid, wlan_intf_opcode_current_connection, None, &mut size, &mut data, None) == 0 && !data.is_null() {
            let attr = &*(data as *const WLAN_CONNECTION_ATTRIBUTES);
            let ssid = &attr.wlanAssociationAttributes.dot11Ssid;
            let n = (ssid.uSSIDLength as usize).min(32);
            out = Some(String::from_utf8_lossy(&ssid.ucSSID[..n]).to_string());
            WlanFreeMemory(data);
        }
        if out.is_some() {
            break;
        }
    }
    WlanFreeMemory(list as *const _);
    out
}

unsafe fn audio_name(en: &IMMDeviceEnumerator) -> Option<String> {
    let d = en.GetDefaultAudioEndpoint(eRender, eConsole).ok()?;
    let store = d.OpenPropertyStore(STGM_READ).ok()?;
    let v = store.GetValue(&PKEY_Device_FriendlyName).ok()?;
    let s = v.to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

#[allow(dead_code)]
fn _unused(_: PCWSTR) {}
