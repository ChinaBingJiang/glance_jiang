//! The wireless networks, as Windows' own WLAN service knows them: read here,
//! so the Wi-Fi page of the system settings has something true to show.
//!
//! It is a component of its own so the settings panel stays about laying rows
//! out, and not about the calls a wireless card answers. A network is joined
//! here too: a saved one from the profile Windows keeps, and a new one by a
//! profile written for it with `WlanSetProfile`, its password kept inside as
//! the key. Enterprise networks, which want a certificate or a domain account,
//! are still led to Windows (see the Wi-Fi page's rows).

use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::{LocalFree, ERROR_SUCCESS, HANDLE, HLOCAL};
use windows::Win32::NetworkManagement::IpHelper::{
    GetAdaptersAddresses, GAA_FLAG_INCLUDE_GATEWAYS, IP_ADAPTER_ADDRESSES_LH,
};
use windows::Win32::NetworkManagement::WiFi::{
    dot11_BSS_type_infrastructure, dot11_radio_state_off, dot11_radio_state_on,
    wlan_connection_mode_profile, wlan_interface_state_connected, wlan_interface_state_not_ready,
    wlan_intf_opcode_channel_number, wlan_intf_opcode_current_connection,
    wlan_intf_opcode_radio_state, WlanCloseHandle, WlanConnect, WlanDeleteProfile, WlanDisconnect,
    WlanEnumInterfaces, WlanFreeMemory, WlanGetAvailableNetworkList, WlanGetProfile,
    WlanGetProfileList, WlanOpenHandle, WlanQueryInterface, WlanSetInterface, WlanSetProfile,
    DOT11_SSID, WLAN_AVAILABLE_NETWORK_CONNECTED, WLAN_AVAILABLE_NETWORK_HAS_PROFILE,
    WLAN_AVAILABLE_NETWORK_LIST, WLAN_CONNECTION_ATTRIBUTES, WLAN_CONNECTION_PARAMETERS,
    WLAN_INTERFACE_INFO_LIST, WLAN_PROFILE_GET_PLAINTEXT_KEY, WLAN_PROFILE_INFO_LIST,
    WLAN_RADIO_STATE,
};
use windows::Win32::Networking::WinSock::{
    AF_INET, AF_INET6, AF_UNSPEC, SOCKADDR_IN, SOCKADDR_IN6, SOCKET_ADDRESS,
};
use windows::Win32::Security::Cryptography::{
    CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
};

/// One network, as the card can see it.
#[derive(Clone, Debug)]
pub struct Net {
    /// Its name.
    pub ssid: String,
    /// The signal, 0..100, as Windows reckons it.
    pub signal: u32,
    /// Whether it asks for a key.
    pub secure: bool,
    /// Whether a profile is kept for it: it has been joined before.
    pub known: bool,
    /// Whether this PC is on it now.
    pub connected: bool,
    /// The name Windows keeps its profile under; empty where there is none yet
    /// (a network never joined on this PC).
    pub profile: String,
    /// How it asks to be let in, as the card reads it (`DOT11_AUTH_ALGORITHM`):
    /// 1 open, 2 shared (WEP), 4 WPA-personal, 7 WPA2-personal, 9 WPA3-personal,
    /// the rest enterprise.
    pub auth: i32,
    /// How it keeps its traffic private, as the card reads it
    /// (`DOT11_CIPHER_ALGORITHM`): 0 none, 2 TKIP, 4 AES, the rest.
    pub cipher: i32,
}

/// Everything the Wi-Fi page shows, as one reading of the wireless card.
#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    /// Whether the card is up at all — its radio on, ready to scan.
    pub on: bool,
    /// The networks it can see, the joined one first, then the known, then the
    /// strongest.
    pub nets: Vec<Net>,
    /// The finer truth of the network this PC is on, when it is on one: read
    /// from the card's current connection (see `details`).
    pub connection: Option<Connection>,
    /// The names Windows keeps profiles under: every network this PC has been
    /// made to join, whether it can be seen now or not (Mac's known networks).
    pub saved: Vec<String>,
    /// The numbers the network has handed this PC, read off the card's own
    /// adapter when there is a connection to read them from.
    pub addr: Option<Address>,
}

/// The current connection, as the card itself reports it — the details Mac shows
/// when you press the info button beside the joined network.
#[derive(Clone, Debug)]
pub struct Connection {
    /// The access point's address, `AA:BB:CC:DD:EE:FF`; empty where unknown.
    pub bssid: String,
    /// The radio's kind, as `802.11ac` or `802.11ax`; empty where unknown.
    pub phy: String,
    /// The band it talks on, as the phy says (a guess where the phy is shared).
    pub band: String,
    /// The link rate in and out, in the card's own units (see `link`).
    pub rx: u32,
    pub tx: u32,
    /// The signal, 0..100, as the association reports it.
    pub signal: u32,
    /// The channel the card sits on, as the card itself is told; 0 where unknown.
    pub channel: u32,
    /// The name the joined network's profile is kept under; empty where the
    /// card will not say.
    pub profile: String,
    /// Whether that profile joins of its own accord when the network returns.
    pub auto: bool,
}

/// The numbers the network hands this PC, read off the adapter the wireless
/// card answers by — the things Mac lists beside the joined network: its
/// address and mask, the router and name servers it was given, and the card's
/// own address.
#[derive(Clone, Debug, Default)]
pub struct Address {
    /// The IPv4 address, dotted; empty where it has none.
    pub ip: String,
    /// Its subnet mask, read from the prefix length the card was given.
    pub mask: String,
    /// The router it reaches out by.
    pub router: String,
    /// The name servers it was given, joined.
    pub dns: String,
    /// The card's own address, `AA:BB:CC:DD:EE:FF`; empty where unknown.
    pub mac: String,
    /// Its first global IPv6 address, compressed; empty where there is none.
    pub v6: String,
    /// Whether the numbers came by themselves (DHCP) or were set by hand.
    pub dhcp: bool,
}

impl Snapshot {
    /// The network this PC is on, if it is on one.
    pub fn current(&self) -> Option<&Net> {
        self.nets.iter().find(|net| net.connected)
    }
}

/// A network's name, from the bytes the card gives; empty where it is not
/// called anything.
fn name(ssid: &DOT11_SSID) -> String {
    let len = (ssid.uSSIDLength as usize).min(ssid.ucSSID.len());
    String::from_utf8_lossy(&ssid.ucSSID[..len]).into_owned()
}

/// Wide units cut at their first zero, as text: a profile's name.
fn wide(units: &[u16]) -> String {
    let len = units
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(units.len());
    String::from_utf16_lossy(&units[..len])
}

/// A handle to ask the wireless service with; `None` where it is not there to
/// answer.
fn open() -> Option<HANDLE> {
    let (mut version, mut client) = (0u32, HANDLE::default());
    (unsafe { WlanOpenHandle(2, None, &mut version, &mut client) } == ERROR_SUCCESS.0)
        .then_some(client)
}

/// Whether any wireless card is up, and the one to ask: the connected card if
/// there is one, else a card that is ready, else the first there is.
fn interface(client: HANDLE) -> (bool, Option<windows::core::GUID>) {
    let mut list = std::ptr::null_mut::<WLAN_INTERFACE_INFO_LIST>();
    if unsafe { WlanEnumInterfaces(client, None, &mut list) } != ERROR_SUCCESS.0 || list.is_null() {
        return (false, None);
    }
    let read = unsafe { &*list };
    let cards = unsafe {
        std::slice::from_raw_parts(read.InterfaceInfo.as_ptr(), read.dwNumberOfItems as usize)
    };
    let on = cards
        .iter()
        .any(|card| card.isState != wlan_interface_state_not_ready);
    let guid = cards
        .iter()
        .find(|card| card.isState == wlan_interface_state_connected)
        .or_else(|| {
            cards
                .iter()
                .find(|card| card.isState != wlan_interface_state_not_ready)
        })
        .or_else(|| cards.first())
        .map(|card| card.InterfaceGuid);
    unsafe { WlanFreeMemory(list as *const _) };
    (on, guid)
}

/// The networks one card can see, gathered: same-named ones are taken as one
/// (the strongest signal kept, and a network remembered if any of them is
/// joined or connected), then the useful ones put first.
fn networks(client: HANDLE, guid: &windows::core::GUID) -> Vec<Net> {
    let mut list = std::ptr::null_mut::<WLAN_AVAILABLE_NETWORK_LIST>();
    if unsafe { WlanGetAvailableNetworkList(client, guid, 0, None, &mut list) } != ERROR_SUCCESS.0
        || list.is_null()
    {
        return Vec::new();
    }
    let read = unsafe { &*list };
    let seen =
        unsafe { std::slice::from_raw_parts(read.Network.as_ptr(), read.dwNumberOfItems as usize) };
    let mut out: Vec<Net> = Vec::new();
    for each in seen {
        let ssid = name(&each.dot11Ssid);
        if ssid.is_empty() {
            continue;
        }
        let net = Net {
            ssid: ssid.clone(),
            signal: each.wlanSignalQuality,
            secure: each.bSecurityEnabled.0 != 0,
            known: each.dwFlags & WLAN_AVAILABLE_NETWORK_HAS_PROFILE != 0,
            connected: each.dwFlags & WLAN_AVAILABLE_NETWORK_CONNECTED != 0,
            profile: wide(&each.strProfileName),
            auth: each.dot11DefaultAuthAlgorithm.0,
            cipher: each.dot11DefaultCipherAlgorithm.0,
        };
        match out.iter_mut().find(|other| other.ssid == ssid) {
            Some(other) => {
                other.connected |= net.connected;
                other.known |= net.known;
                other.secure |= net.secure;
                if net.signal > other.signal {
                    other.auth = net.auth;
                    other.cipher = net.cipher;
                }
                other.signal = other.signal.max(net.signal);
                if other.profile.is_empty() {
                    other.profile = net.profile;
                }
            }
            None => out.push(net),
        }
    }
    unsafe { WlanFreeMemory(list as *const _) };
    out.sort_by(|a, b| {
        b.connected
            .cmp(&a.connected)
            .then(b.known.cmp(&a.known))
            .then(b.signal.cmp(&a.signal))
    });
    out
}

/// The wireless card read once, whole: whether it is on, what it can see, and —
/// when it is on a network — the details of that connection, the numbers the
/// network has handed it, and every profile kept for networks out of sight.
pub fn scan() -> Snapshot {
    let Some(client) = open() else {
        return Snapshot::default();
    };
    let (on, guid) = interface(client);
    let (nets, connection, saved, addr) = match guid {
        Some(guid) if on => {
            let nets = networks(client, &guid);
            let connection = nets
                .iter()
                .any(|net| net.connected)
                .then(|| details(client, &guid))
                .flatten();
            let saved = profiles(client, &guid);
            let addr = connection.as_ref().and_then(|_| address(&guid));
            (nets, connection, saved, addr)
        }
        Some(guid) => (Vec::new(), None, profiles(client, &guid), None),
        _ => (Vec::new(), None, Vec::new(), None),
    };
    unsafe {
        let _ = WlanCloseHandle(client, None);
    }
    Snapshot {
        on,
        nets,
        connection,
        saved,
        addr,
    }
}

/// The details of the connection now up, read from the card's current-connection
/// attributes: the access point's address, the radio's kind and band, the two
/// link rates, and the signal. `None` where the card will not say.
fn details(client: HANDLE, guid: &windows::core::GUID) -> Option<Connection> {
    let (mut size, mut told) = (0u32, std::ptr::null_mut::<std::ffi::c_void>());
    let asked = unsafe {
        WlanQueryInterface(
            client,
            guid,
            wlan_intf_opcode_current_connection,
            None,
            &mut size,
            &mut told,
            None,
        )
    };
    if asked != ERROR_SUCCESS.0 || told.is_null() {
        return None;
    }
    let attrs = unsafe { *(told as *const WLAN_CONNECTION_ATTRIBUTES) };
    unsafe { WlanFreeMemory(told) };
    let assoc = attrs.wlanAssociationAttributes;
    let bssid = assoc
        .dot11Bssid
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<Vec<_>>()
        .join(":");
    let phy = phy_name(assoc.dot11PhyType.0);
    let band = band_of(&phy).to_string();
    let profile = wide(&attrs.strProfileName);
    let auto = profile.is_empty() || joins_of_its_own(client, guid, &profile);
    Some(Connection {
        bssid,
        phy: phy.to_string(),
        band,
        rx: assoc.ulRxRate,
        tx: assoc.ulTxRate,
        signal: assoc.wlanSignalQuality,
        channel: channel(client, guid),
        profile,
        auto,
    })
}

/// The channel the card sits on, asked of the card itself; 0 where it will not
/// say (the answer is one number, a `ULONG`).
fn channel(client: HANDLE, guid: &windows::core::GUID) -> u32 {
    let (mut size, mut told) = (0u32, std::ptr::null_mut::<std::ffi::c_void>());
    let asked = unsafe {
        WlanQueryInterface(
            client,
            guid,
            wlan_intf_opcode_channel_number,
            None,
            &mut size,
            &mut told,
            None,
        )
    };
    if asked != ERROR_SUCCESS.0 || told.is_null() {
        return 0;
    }
    let number = unsafe { *(told as *const u32) };
    unsafe { WlanFreeMemory(told) };
    number
}

/// The profile's XML, as Windows keeps it; `None` where the card or the name
/// is not there to answer. The key inside stays as Windows keeps it — the page
/// never asks to see it.
fn profile_xml(client: HANDLE, guid: &windows::core::GUID, profile: &str) -> Option<String> {
    let wide: Vec<u16> = profile.encode_utf16().chain([0]).collect();
    let mut xml = PWSTR::null();
    if unsafe {
        WlanGetProfile(
            client,
            guid,
            PCWSTR(wide.as_ptr()),
            None,
            &mut xml,
            None,
            None,
        )
    } != ERROR_SUCCESS.0
    {
        return None;
    }
    let text = unsafe { xml.to_string().ok() }.filter(|t| !t.is_empty());
    unsafe { WlanFreeMemory(xml.0 as *const _) };
    text
}

/// Whether a kept profile joins of its own accord: its XML says `auto` where
/// the connection mode is written.
fn joins_of_its_own(client: HANDLE, guid: &windows::core::GUID, profile: &str) -> bool {
    profile_xml(client, guid, profile)
        .is_some_and(|xml| xml.contains("<connectionMode>auto</connectionMode>"))
}

/// Turn a kept profile's joining on or off: its XML is read, the connection
/// mode written anew in it, and the profile set back as it came. Whether
/// Windows took it; nothing else in the XML — the key included — is changed.
pub fn set_auto_join(profile: &str, on: bool) -> bool {
    if profile.is_empty() {
        return false;
    }
    let (from, to) = if on {
        (
            "<connectionMode>manual</connectionMode>",
            "<connectionMode>auto</connectionMode>",
        )
    } else {
        (
            "<connectionMode>auto</connectionMode>",
            "<connectionMode>manual</connectionMode>",
        )
    };
    let Some(client) = open() else {
        return false;
    };
    let guid = match interface(client).1 {
        Some(guid) => guid,
        None => {
            unsafe {
                let _ = WlanCloseHandle(client, None);
            }
            return false;
        }
    };
    let done = match profile_xml(client, &guid, profile) {
        Some(xml) if xml.contains(from) || xml.contains(to) => {
            let edited = xml.replace(from, to);
            let xml_wide: Vec<u16> = edited.encode_utf16().chain([0]).collect();
            let mut reason = 0u32;
            unsafe {
                WlanSetProfile(
                    client,
                    &guid,
                    0,
                    PCWSTR(xml_wide.as_ptr()),
                    PCWSTR::null(),
                    true,
                    None,
                    &mut reason,
                ) == ERROR_SUCCESS.0
            }
        }
        _ => false,
    };
    unsafe {
        let _ = WlanCloseHandle(client, None);
    }
    done
}

/// Every profile the card keeps, by name: the networks this PC has been made
/// to join, whether they can be seen now or not.
fn profiles(client: HANDLE, guid: &windows::core::GUID) -> Vec<String> {
    let mut list = std::ptr::null_mut::<WLAN_PROFILE_INFO_LIST>();
    if unsafe { WlanGetProfileList(client, guid, None, &mut list) } != ERROR_SUCCESS.0
        || list.is_null()
    {
        return Vec::new();
    }
    let read = unsafe { &*list };
    let kept = unsafe {
        std::slice::from_raw_parts(read.ProfileInfo.as_ptr(), read.dwNumberOfItems as usize)
    };
    let mut out: Vec<String> = Vec::new();
    for each in kept {
        let name = wide(&each.strProfileName);
        if !name.is_empty() && !out.contains(&name) {
            out.push(name);
        }
    }
    unsafe { WlanFreeMemory(list as *const _) };
    out
}

/// The numbers handed this PC, read off the adapter whose own number is the
/// wireless card's: `GetAdaptersAddresses` walked to the one the guid names
/// (a card's LUID is the last eight bytes of its guid). `None` where the
/// adapter is not among those listed.
pub fn address(guid: &windows::core::GUID) -> Option<Address> {
    let want = u64::from_le_bytes(guid.data4);
    let mut buf: Vec<u64> = Vec::new();
    let mut size = 0u32;
    loop {
        let list = buf.as_mut_ptr() as *mut IP_ADAPTER_ADDRESSES_LH;
        let status = unsafe {
            GetAdaptersAddresses(
                AF_UNSPEC.0 as u32,
                GAA_FLAG_INCLUDE_GATEWAYS,
                None,
                Some(list),
                &mut size,
            )
        };
        if status == ERROR_BUFFER_OVERFLOW {
            buf.resize((size as usize).div_ceil(8), 0);
            continue;
        }
        if status != ERROR_SUCCESS.0 {
            return None;
        }
        let mut here = list as *const IP_ADAPTER_ADDRESSES_LH;
        while !here.is_null() {
            let a = unsafe { &*here };
            if unsafe { a.Luid.Value } == want {
                let mut out = Address::default();
                let mut at = a.FirstUnicastAddress;
                while !at.is_null() {
                    let node = unsafe { &*at };
                    if let Some((said, v4)) = told(node.Address) {
                        if v4 && out.ip.is_empty() {
                            out.ip = said;
                            out.mask = mask(node.OnLinkPrefixLength);
                        } else if !v4 && out.v6.is_empty() && !said.starts_with("fe80") {
                            out.v6 = said;
                        }
                    }
                    at = unsafe { (*at).Next };
                }
                let mut at = a.FirstGatewayAddress;
                while !at.is_null() {
                    if let Some((said, true)) = told(unsafe { (*at).Address }) {
                        out.router = said;
                        break;
                    }
                    at = unsafe { (*at).Next };
                }
                let mut at = a.FirstDnsServerAddress;
                let mut servers = Vec::new();
                while !at.is_null() {
                    if let Some((said, _)) = told(unsafe { (*at).Address }) {
                        servers.push(said);
                    }
                    at = unsafe { (*at).Next };
                }
                out.dns = servers.join("   ");
                let len = (a.PhysicalAddressLength as usize).min(a.PhysicalAddress.len());
                out.mac = a.PhysicalAddress[..len]
                    .iter()
                    .map(|byte| format!("{byte:02X}"))
                    .collect::<Vec<_>>()
                    .join(":");
                // `IP_ADAPTER_DHCP_ENABLED`, as the flags word keeps it.
                out.dhcp = unsafe { a.Anonymous2.Flags } & 0x40 != 0;
                return Some(out);
            }
            here = unsafe { (*here).Next };
        }
        return None;
    }
}

/// `GetAdaptersAddresses` answering that the buffer was too small — the one
/// number it gives besides success, as `crate::sys` reads it too.
const ERROR_BUFFER_OVERFLOW: u32 = 111;

/// A socket address as text, and whether it speaks IPv4: four numbers, or a
/// compressed IPv6. `None` where it is neither.
fn told(address: SOCKET_ADDRESS) -> Option<(String, bool)> {
    if address.lpSockaddr.is_null() {
        return None;
    }
    let family = unsafe { (*(address.lpSockaddr as *const SOCKADDR_IN)).sin_family };
    if family == AF_INET {
        let socket = address.lpSockaddr as *const SOCKADDR_IN;
        let octets = unsafe { (*socket).sin_addr.S_un.S_addr }.to_ne_bytes();
        Some((
            format!("{}.{}.{}.{}", octets[0], octets[1], octets[2], octets[3]),
            true,
        ))
    } else if family == AF_INET6 {
        let socket = address.lpSockaddr as *const SOCKADDR_IN6;
        let bytes = unsafe { (*socket).sin6_addr.u.Byte };
        Some((colon(&bytes), false))
    } else {
        None
    }
}

/// Sixteen bytes of IPv6, said as it is written: groups of four hex digits,
/// leading zeros dropped, the longest run of empty groups cut to `::`.
fn colon(bytes: &[u8; 16]) -> String {
    let words: Vec<u16> = (0..8)
        .map(|i| u16::from_be_bytes([bytes[2 * i], bytes[2 * i + 1]]))
        .collect();
    if words.iter().all(|word| *word == 0) {
        return "::".to_string();
    }
    let (mut best, mut best_at, mut run, mut at) = (0usize, usize::MAX, 0usize, usize::MAX);
    for (i, word) in words.iter().enumerate() {
        if *word == 0 {
            if run == 0 {
                at = i;
            }
            run += 1;
            if run > best {
                best = run;
                best_at = at;
            }
        } else {
            run = 0;
        }
    }
    if best < 2 {
        best_at = usize::MAX;
    }
    let mut parts: Vec<String> = Vec::new();
    if best_at == 0 {
        // The run opens the address: it begins with `::`.
        parts.push(String::new());
        parts.push(String::new());
    }
    let mut i = if best_at == 0 { best } else { 0 };
    while i < 8 {
        if i == best_at {
            i += best;
            parts.push(String::new());
        } else {
            parts.push(format!("{:x}", words[i]));
            i += 1;
        }
    }
    if best_at != usize::MAX && best_at + best == 8 {
        // The run closes the address: it ends with `::`, not a lone `:`.
        parts.push(String::new());
    }
    parts.join(":")
}

/// A subnet mask from the prefix length it was given by: twenty-four bits on,
/// four numbers as they come.
fn mask(prefix: u8) -> String {
    if prefix > 32 {
        return String::new();
    }
    let bits = if prefix == 0 {
        0
    } else {
        u32::MAX << (32 - prefix)
    };
    let octets = bits.to_be_bytes();
    format!("{}.{}.{}.{}", octets[0], octets[1], octets[2], octets[3])
}

/// The radio's kind from the phy number Windows gives; empty where it is one the
/// card does not name here.
fn phy_name(phy: i32) -> &'static str {
    match phy {
        2 => "802.11b",
        4 => "802.11a",
        6 => "802.11g",
        7 => "802.11n",
        8 => "802.11ac",
        10 => "802.11ax",
        11 => "802.11be",
        9 => "802.11ad",
        _ => "",
    }
}

/// The band to be read off the radio's kind, as far as the kind can say it.
fn band_of(phy: &str) -> &'static str {
    match phy {
        "802.11b" | "802.11g" => "2.4 GHz",
        "802.11a" | "802.11ac" => "5 GHz",
        "802.11n" => "2.4 / 5 GHz",
        "802.11ax" => "2.4 / 5 / 6 GHz",
        "802.11be" => "2.4 / 5 / 6 GHz",
        "802.11ad" => "60 GHz",
        _ => "",
    }
}

/// A link rate, in the card's own units, said as megabits a second. The card may
/// tell it in bits or kilobits a second; either way it is shown as it means.
pub fn link(mbits: u32) -> String {
    if mbits == 0 {
        return String::new();
    }
    let m = if mbits >= 1_000_000 {
        mbits as f64 / 1_000_000.0
    } else {
        mbits as f64 / 1_000.0
    };
    if m >= 10.0 {
        format!("{:.0} Mbps", m)
    } else {
        format!("{:.1} Mbps", m)
    }
}

/// Let go of the network this PC is on; whether Windows did it.
pub fn disconnect() -> bool {
    let Some(client) = open() else {
        return false;
    };
    let done = match interface(client).1 {
        Some(guid) => unsafe { WlanDisconnect(client, &guid, None) == ERROR_SUCCESS.0 },
        None => false,
    };
    unsafe {
        let _ = WlanCloseHandle(client, None);
    }
    done
}

/// The wireless card's radio, turned on or off. Windows asks for the
/// administrator's leave for this: without it nothing changes, and this says
/// so — the page then shows the card as it still is.
pub fn set_radio(on: bool) -> bool {
    let Some(client) = open() else {
        return false;
    };
    let guid = match interface(client).1 {
        Some(guid) => guid,
        None => {
            unsafe {
                let _ = WlanCloseHandle(client, None);
            }
            return false;
        }
    };
    // How many radios the card has is asked first, so each one's switch can be
    // set as it comes back; where that is not told, one radio is assumed.
    let (mut size, mut told) = (0u32, std::ptr::null_mut::<std::ffi::c_void>());
    let asked = unsafe {
        WlanQueryInterface(
            client,
            &guid,
            wlan_intf_opcode_radio_state,
            None,
            &mut size,
            &mut told,
            None,
        )
    };
    let mut state = WLAN_RADIO_STATE::default();
    if asked == ERROR_SUCCESS.0 && !told.is_null() {
        state = unsafe { *(told as *const WLAN_RADIO_STATE) };
        unsafe { WlanFreeMemory(told) };
    }
    if state.dwNumberOfPhys == 0 {
        state.dwNumberOfPhys = 1;
    }
    let radio = if on {
        dot11_radio_state_on
    } else {
        dot11_radio_state_off
    };
    let count = (state.dwNumberOfPhys as usize).min(state.PhyRadioState.len());
    for each in state.PhyRadioState.iter_mut().take(count) {
        each.dot11SoftwareRadioState = radio;
    }
    let done = unsafe {
        WlanSetInterface(
            client,
            &guid,
            wlan_intf_opcode_radio_state,
            std::mem::size_of::<WLAN_RADIO_STATE>() as u32,
            &state as *const _ as *const std::ffi::c_void,
            None,
        ) == ERROR_SUCCESS.0
    };
    unsafe {
        let _ = WlanCloseHandle(client, None);
    }
    done
}

/// Join the network whose profile is kept under `profile` (a saved network, so
/// Windows already has its password); whether it got as far as asking. Nothing
/// is waited for — the page reads the card again to see where it came out.
pub fn connect(profile: &str) -> bool {
    if profile.is_empty() {
        return false;
    }
    let Some(client) = open() else {
        return false;
    };
    let guid = interface(client).1;
    let wide: Vec<u16> = profile.encode_utf16().chain([0]).collect();
    let done = guid.is_some_and(|guid| {
        let params = WLAN_CONNECTION_PARAMETERS {
            wlanConnectionMode: wlan_connection_mode_profile,
            strProfile: PCWSTR(wide.as_ptr()),
            pDot11Ssid: std::ptr::null_mut(),
            pDesiredBssidList: std::ptr::null_mut(),
            dot11BssType: dot11_BSS_type_infrastructure,
            dwFlags: 0,
        };
        unsafe { WlanConnect(client, &guid, &params, None) == ERROR_SUCCESS.0 }
    });
    unsafe {
        let _ = WlanCloseHandle(client, None);
    }
    done
}

/// Let go of a network for good: its saved profile deleted, so the PC will not
/// rejoin it; whether Windows did it.
pub fn forget(profile: &str) -> bool {
    if profile.is_empty() {
        return false;
    }
    let Some(client) = open() else {
        return false;
    };
    let guid = interface(client).1;
    let wide: Vec<u16> = profile.encode_utf16().chain([0]).collect();
    let done = guid.is_some_and(|guid| unsafe {
        WlanDeleteProfile(client, &guid, PCWSTR(wide.as_ptr()), None) == ERROR_SUCCESS.0
    });
    unsafe {
        let _ = WlanCloseHandle(client, None);
    }
    done
}

/// What the service said when asked for a network's kept password.
pub enum Key {
    /// The password itself, as it was kept.
    Given(String),
    /// The profile is there but keeps no password: an open network.
    None,
    /// The service would not answer at all — the profile is gone, or the
    /// call failed. The saved key itself needs no administrator to read.
    Refused,
}

/// The password Windows keeps for a saved network, as Mac will show one after
/// a login. The service hands out the profile's XML to any program of the
/// signing-in user, the same as `netsh` does; the key inside it comes locked
/// as a DPAPI blob, and this unlocks it as the user whose login it is — no
/// administrator's leave wanted. A `Refused` here means the lock would not
/// open or the asking failed, which is not the common way.
pub fn password(profile: &str) -> Key {
    if profile.is_empty() {
        return Key::Refused;
    }
    let Some(client) = open() else {
        return Key::Refused;
    };
    let guid = interface(client).1;
    let answer = guid.map(|guid| {
        let wide: Vec<u16> = profile.encode_utf16().chain([0]).collect();
        let mut xml = PWSTR::null();
        // Ask for the plaintext key going in — `WLAN_PROFILE_GET_PLAINTEXT_KEY`,
        // which is four, not one; the service reads the bit and answers the
        // XML in clear to those it lets, the same ask `netsh` makes.
        let mut flags = WLAN_PROFILE_GET_PLAINTEXT_KEY;
        let asked = unsafe {
            WlanGetProfile(
                client,
                &guid,
                PCWSTR(wide.as_ptr()),
                None,
                &mut xml,
                Some(&mut flags),
                None,
            )
        };
        if asked != ERROR_SUCCESS.0 {
            return Key::Refused;
        }
        let text = unsafe { xml.to_string().ok() };
        unsafe { WlanFreeMemory(xml.0 as *const _) };
        let key = text.and_then(|xml| {
            let rest = xml.split("<keyMaterial>").nth(1)?;
            let key = rest.split("</keyMaterial>").next().map(xml_unescape);
            key.filter(|key| !key.is_empty())
        });
        match key {
            // The service answers in clear when it lets the asking; when it
            // keeps the key locked it hands out its DPAPI blob instead, and
            // the lock of a profile kept under this login opens here too.
            Some(key) if looks_encrypted(&key) => match unprotect(&key) {
                Some(plain) => Key::Given(plain),
                None => Key::Refused,
            },
            Some(key) => Key::Given(key),
            None => Key::None, // answered, and keeps no key: an open network
        }
    });
    unsafe {
        let _ = WlanCloseHandle(client, None);
    }
    answer.unwrap_or(Key::Refused)
}

/// Text taken out of the profile's XML put back as it was written.
fn xml_unescape(text: &str) -> String {
    text.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

/// Whether the key the service gave is really its DPAPI blob and not the
/// password itself: a long unbroken run of hex digits, which no WPA password
/// of 63 characters or fewer can be.
fn looks_encrypted(key: &str) -> bool {
    key.len() >= 128 && key.chars().all(|c| c.is_ascii_hexdigit())
}

/// The blob's hex read back as bytes.
fn unhex(text: &str) -> Option<Vec<u8>> {
    let raw = text.as_bytes();
    if raw.len() % 2 != 0 {
        return None;
    }
    let digit = |c: u8| match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    };
    let mut bytes = Vec::with_capacity(raw.len() / 2);
    for pair in raw.chunks(2) {
        let hi = digit(pair[0])?;
        let lo = digit(pair[1])?;
        bytes.push(hi << 4 | lo);
    }
    Some(bytes)
}

/// A kept password unlocked from the service's blob, the way it was locked:
/// DPAPI under this login's own keys, which the user's programs may open and
/// an administrator's leave has nothing to do with.
fn unprotect(blob: &str) -> Option<String> {
    let bytes = unhex(blob)?;
    let locked = CRYPT_INTEGER_BLOB {
        cbData: bytes.len() as u32,
        pbData: bytes.as_ptr() as *mut u8,
    };
    let mut opened = CRYPT_INTEGER_BLOB::default();
    let gave = unsafe {
        CryptUnprotectData(
            &locked,
            None,
            None,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut opened,
        )
    };
    if gave.is_err() {
        return None;
    }
    let plain =
        unsafe { std::slice::from_raw_parts(opened.pbData, opened.cbData as usize).to_vec() };
    unsafe {
        let _ = LocalFree(Some(HLOCAL(opened.pbData as *mut _)));
    }
    let text = String::from_utf8(plain)
        .unwrap_or_else(|kept| String::from_utf8_lossy(kept.as_bytes()).into_owned());
    Some(text)
}

/// Whether the page can join such a network on its own: an open one, or a
/// personal one that wants only a single password (WEP, WPA, WPA2, WPA3).
/// Enterprise networks, which want a certificate or a domain account, are not
/// among them and are led to Windows.
pub fn page_joinable(auth: i32, secure: bool) -> bool {
    !secure || matches!(auth, 2 | 4 | 7 | 9)
}

/// Whether joining such a network wants a password typed in: a secured one of
/// the kinds the page can join.
pub fn needs_key(auth: i32, secure: bool) -> bool {
    secure && matches!(auth, 2 | 4 | 7 | 9)
}

/// Text made safe to sit inside the profile's XML.
fn xml_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(ch),
        }
    }
    out
}

/// Join a network the page can join itself: a profile is written for it with
/// `WlanSetProfile` — the password kept inside as its key — and the card is
/// asked to connect to that profile. An open network needs no password. Whether
/// it got as far as asking; nothing is waited for, the page reads the card
/// again to see where it came out.
pub fn connect_new(ssid: &str, auth: i32, cipher: i32, secure: bool, password: &str) -> bool {
    if ssid.is_empty() || !page_joinable(auth, secure) {
        return false;
    }
    let authentication = match (auth, secure) {
        (_, false) | (1, _) => "open",
        (2, _) => "shared",
        (4, _) => "WPAPSK",
        _ => "WPA2PSK", // 7 WPA2-personal, 9 WPA3-personal (mixed mode), the rest
    };
    let encryption = match authentication {
        "open" => "none",
        "shared" => "WEP",
        _ => match cipher {
            2 => "TKIP",
            _ => "AES",
        },
    };
    let name = xml_escape(ssid);
    let key = if secure {
        format!(
            "<sharedKey><keyType>networkKey</keyType><keyMaterial>{}</keyMaterial></sharedKey>",
            xml_escape(password)
        )
    } else {
        String::new()
    };
    let xml = format!(
        "<?xml version=\"1.0\"?>\n\
         <WLANProfile xmlns=\"http://www.microsoft.com/networking/WLAN/profile/v1\">\
         <name>{name}</name><SSIDConfig><SSID><name>{name}</name></SSID></SSIDConfig>\
         <connectionType>ESS</connectionType><connectionMode>auto</connectionMode>\
         <MSM><security><authEncryption><authentication>{authentication}</authentication>\
         <encryption>{encryption}</encryption><useOneX>false</useOneX></authEncryption>{key}\
         </security></MSM></WLANProfile>"
    );
    let Some(client) = open() else {
        return false;
    };
    let guid = interface(client).1;
    let xml_wide: Vec<u16> = xml.encode_utf16().chain([0]).collect();
    let profile_wide: Vec<u16> = ssid.encode_utf16().chain([0]).collect();
    let done = guid.is_some_and(|guid| unsafe {
        let mut reason = 0u32;
        let set = WlanSetProfile(
            client,
            &guid,
            0,
            PCWSTR(xml_wide.as_ptr()),
            PCWSTR::null(),
            true,
            None,
            &mut reason,
        );
        if set != ERROR_SUCCESS.0 {
            return false;
        }
        let params = WLAN_CONNECTION_PARAMETERS {
            wlanConnectionMode: wlan_connection_mode_profile,
            strProfile: PCWSTR(profile_wide.as_ptr()),
            pDot11Ssid: std::ptr::null_mut(),
            pDesiredBssidList: std::ptr::null_mut(),
            dot11BssType: dot11_BSS_type_infrastructure,
            dwFlags: 0,
        };
        WlanConnect(client, &guid, &params, None) == ERROR_SUCCESS.0
    });
    unsafe {
        let _ = WlanCloseHandle(client, None);
    }
    done
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Asks this PC's service for the keys it keeps — run by hand, with
    /// `--ignored`, to see where a refusal really comes from.
    #[test]
    #[ignore = "reads this PC's saved network passwords"]
    fn gives_up_the_keys_it_keeps() {
        let snap = scan();
        for name in &snap.saved {
            let said = match password(name) {
                Key::Given(key) => {
                    let count = key.chars().count();
                    let head: String = key.chars().take(4).collect();
                    let tail: String = key.chars().rev().take(4).collect();
                    format!("Given ({count} chars, {head}..{tail})")
                }
                Key::None => "None".to_string(),
                Key::Refused => "Refused".to_string(),
            };
            println!("{name}: {said}");
        }
        assert!(
            !snap.saved.is_empty(),
            "this PC keeps no profiles to ask for"
        );
        // The raw asking too, that the error code and the flags answer show.
        let client = open().expect("the service answers");
        let guid = interface(client).1.expect("a card is there");
        let name = snap.saved[0].clone();
        let wide: Vec<u16> = name.encode_utf16().chain([0]).collect();
        let mut xml = PWSTR::null();
        let mut flags = WLAN_PROFILE_GET_PLAINTEXT_KEY;
        let asked = unsafe {
            WlanGetProfile(
                client,
                &guid,
                PCWSTR(wide.as_ptr()),
                None,
                &mut xml,
                Some(&mut flags),
                None,
            )
        };
        println!("raw {name}: code {asked} flags {flags:#x}");
        if !xml.is_null() {
            let text = unsafe { xml.to_string().unwrap_or_default() };
            println!("has keyMaterial: {}", text.contains("keyMaterial"));
            unsafe { WlanFreeMemory(xml.0 as *const _) };
        }
        unsafe {
            let _ = WlanCloseHandle(client, None);
        }
    }

    #[test]
    fn a_kept_lock_is_known_by_its_hex() {
        // A password in clear is never a long unbroken run of hex digits,
        // however the blob always is.
        assert!(!looks_encrypted("981573.."));
        assert!(looks_encrypted(&"0a1b".repeat(40)));
        // A short hexy password stays the password.
        assert!(!looks_encrypted("dead"));
    }

    #[test]
    fn the_blobs_hex_reads_back_as_bytes() {
        assert_eq!(unhex("01FF7a").unwrap(), vec![0x01, 0xff, 0x7a]);
        assert!(unhex("010").is_none()); // an odd run pairs no bytes
        assert!(unhex("0g").is_none()); // and no non-digit sneaks through
    }

    #[test]
    fn masks_come_from_prefix_lengths() {
        assert_eq!(mask(24), "255.255.255.0");
        assert_eq!(mask(16), "255.255.0.0");
        assert_eq!(mask(0), "0.0.0.0");
        assert_eq!(mask(32), "255.255.255.255");
        assert_eq!(mask(33), "");
    }

    #[test]
    fn six_is_said_short() {
        // A plain address, no run to cut.
        assert_eq!(
            colon(&[1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]),
            "100::1"
        );
        // The longest run of empty groups goes to `::`.
        assert_eq!(
            colon(&[0x20, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]),
            "2001::1"
        );
        // Link-local, as it comes.
        assert_eq!(
            colon(&[0xfe, 0x80, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 5]),
            "fe80::5"
        );
        // All empty: the unspecified address.
        assert_eq!(colon(&[0; 16]), "::");
    }

    #[test]
    fn link_rates_are_said_in_megabits() {
        assert_eq!(link(0), "");
        assert_eq!(link(72_000), "72 Mbps");
        assert_eq!(link(9_600), "9.6 Mbps");
        assert_eq!(link(1_441_000_000), "1441 Mbps");
    }
}
