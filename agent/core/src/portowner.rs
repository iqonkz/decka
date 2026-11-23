//! Who listens on a TCP port (T21.16): the name of the program that keeps the port of the agent busy.

/// One row of the table of listening sockets: the local port as Windows stores it (network byte order in the low 16 bits).
pub type Row = (u32, u32);

/// The process id of the listener of `port` among `rows` (`(dwLocalPort, dwOwningPid)`).
pub fn pid_of_port(rows: &[Row], port: u16) -> Option<u32> {
    rows.iter().find(|(raw, _)| u16::from_be((*raw & 0xFFFF) as u16) == port).map(|(_, pid)| *pid)
}

/// The exe name of the program that listens on `port` (IPv4), or `None` when it is not known.
#[cfg(windows)]
pub fn port_owner(port: u16) -> Option<String> {
    use windows::Win32::NetworkManagement::IpHelper::{GetExtendedTcpTable, MIB_TCPTABLE_OWNER_PID, TCP_TABLE_OWNER_PID_LISTENER};
    use windows::Win32::Networking::WinSock::AF_INET;

    let af = AF_INET.0 as u32;
    let mut size = 0u32;
    // The first call only tells how big the table is.
    unsafe { GetExtendedTcpTable(None, &mut size, false, af, TCP_TABLE_OWNER_PID_LISTENER, 0) };
    if size == 0 {
        return None;
    }
    let mut buf = vec![0u8; size as usize + 256];
    size = buf.len() as u32;
    let rc = unsafe { GetExtendedTcpTable(Some(buf.as_mut_ptr().cast()), &mut size, false, af, TCP_TABLE_OWNER_PID_LISTENER, 0) };
    if rc != 0 {
        return None;
    }
    let rows: Vec<Row> = unsafe {
        let table = &*(buf.as_ptr() as *const MIB_TCPTABLE_OWNER_PID);
        std::slice::from_raw_parts(table.table.as_ptr(), table.dwNumEntries as usize).iter().map(|r| (r.dwLocalPort, r.dwOwningPid)).collect()
    };
    let pid = pid_of_port(&rows, port)?;
    let spid = sysinfo::Pid::from_u32(pid);
    let mut sys = sysinfo::System::new().ok()?;
    sys.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[spid]), true);
    sys.process(spid).map(|p| p.name().to_string_lossy().to_string())
}

#[cfg(not(windows))]
pub fn port_owner(_port: u16) -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_port_is_read_from_network_byte_order() {
        // 9247 = 0x241F; Windows keeps it as 0x1F24 in the low bytes.
        let raw = u32::from(9247u16.to_be());
        assert_eq!(pid_of_port(&[(u32::from(80u16.to_be()), 4), (raw, 1234)], 9247), Some(1234));
        assert_eq!(pid_of_port(&[(raw, 1234)], 9248), None);
        assert_eq!(pid_of_port(&[], 9247), None);
    }

    /// Loopback only: this very test process listens, and it is found by name.
    #[cfg(windows)]
    #[test]
    fn a_listener_on_loopback_is_found_by_name() {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        let name = port_owner(port).expect("the owner is known");
        assert!(name.to_lowercase().ends_with(".exe"), "{name}");
    }
}
