//! Where broadcasts must go. macOS (and some Linux setups) refuse to send to
//! the limited broadcast address 255.255.255.255 ("no route to host"), so a
//! leader sends to each active interface's subnet-directed broadcast instead,
//! such as 192.168.1.255.

use std::net::Ipv4Addr;

/// Subnet broadcast addresses of the active, non-loopback IPv4 interfaces.
/// Falls back to 255.255.255.255 where interfaces cannot be listed (it works
/// on Windows) or none are up.
pub fn broadcast_addresses() -> Vec<Ipv4Addr> {
    let mut found = list();
    found.sort();
    found.dedup();
    if found.is_empty() {
        found.push(Ipv4Addr::BROADCAST);
    }
    found
}

/// Broadcast address of `address` within `netmask`.
pub fn subnet_broadcast(address: Ipv4Addr, netmask: Ipv4Addr) -> Ipv4Addr {
    Ipv4Addr::from(u32::from(address) | !u32::from(netmask))
}

#[cfg(unix)]
fn list() -> Vec<Ipv4Addr> {
    let mut out = Vec::new();
    let mut head: *mut libc::ifaddrs = std::ptr::null_mut();
    // SAFETY: getifaddrs fills a linked list we only read, then free once.
    unsafe {
        if libc::getifaddrs(&mut head) != 0 {
            return out;
        }
        let mut entry = head;
        while !entry.is_null() {
            let ifa = &*entry;
            entry = ifa.ifa_next;
            let flags = ifa.ifa_flags as libc::c_int;
            let wanted = flags & libc::IFF_UP != 0
                && flags & libc::IFF_BROADCAST != 0
                && flags & libc::IFF_LOOPBACK == 0;
            if !wanted || ifa.ifa_addr.is_null() || ifa.ifa_netmask.is_null() {
                continue;
            }
            if i32::from((*ifa.ifa_addr).sa_family) != libc::AF_INET {
                continue;
            }
            let address = &*(ifa.ifa_addr as *const libc::sockaddr_in);
            let netmask = &*(ifa.ifa_netmask as *const libc::sockaddr_in);
            out.push(subnet_broadcast(
                Ipv4Addr::from(u32::from_be(address.sin_addr.s_addr)),
                Ipv4Addr::from(u32::from_be(netmask.sin_addr.s_addr)),
            ));
        }
        libc::freeifaddrs(head);
    }
    out
}

#[cfg(not(unix))]
fn list() -> Vec<Ipv4Addr> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subnet_broadcast_sets_the_host_bits() {
        assert_eq!(
            subnet_broadcast(
                Ipv4Addr::new(192, 168, 12, 28),
                Ipv4Addr::new(255, 255, 255, 0)
            ),
            Ipv4Addr::new(192, 168, 12, 255)
        );
        assert_eq!(
            subnet_broadcast(Ipv4Addr::new(10, 1, 2, 3), Ipv4Addr::new(255, 255, 0, 0)),
            Ipv4Addr::new(10, 1, 255, 255)
        );
        assert!(!broadcast_addresses().is_empty());
    }
}
