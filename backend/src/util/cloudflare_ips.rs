use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// Cloudflare's published IPv4 ranges (https://www.cloudflare.com/ips/).
pub(crate) const CLOUDFLARE_IPV4: &[(Ipv4Addr, u8)] = &[
    (Ipv4Addr::new(173, 245, 48, 0), 20),
    (Ipv4Addr::new(103, 21, 244, 0), 22),
    (Ipv4Addr::new(103, 22, 200, 0), 22),
    (Ipv4Addr::new(103, 31, 4, 0), 22),
    (Ipv4Addr::new(141, 101, 64, 0), 18),
    (Ipv4Addr::new(108, 162, 192, 0), 18),
    (Ipv4Addr::new(190, 93, 240, 0), 20),
    (Ipv4Addr::new(188, 114, 96, 0), 20),
    (Ipv4Addr::new(197, 234, 240, 0), 22),
    (Ipv4Addr::new(198, 41, 128, 0), 17),
    (Ipv4Addr::new(162, 158, 0, 0), 15),
    (Ipv4Addr::new(104, 16, 0, 0), 13),
    (Ipv4Addr::new(104, 24, 0, 0), 14),
    (Ipv4Addr::new(172, 64, 0, 0), 13),
    (Ipv4Addr::new(131, 0, 72, 0), 22),
];

/// Cloudflare's published IPv6 ranges.
pub(crate) const CLOUDFLARE_IPV6: &[(Ipv6Addr, u8)] = &[
    (Ipv6Addr::new(0x2400, 0xcb00, 0, 0, 0, 0, 0, 0), 32),
    (Ipv6Addr::new(0x2606, 0x4700, 0, 0, 0, 0, 0, 0), 32),
    (Ipv6Addr::new(0x2803, 0xf800, 0, 0, 0, 0, 0, 0), 32),
    (Ipv6Addr::new(0x2405, 0xb500, 0, 0, 0, 0, 0, 0), 32),
    (Ipv6Addr::new(0x2405, 0x8100, 0, 0, 0, 0, 0, 0), 32),
    (Ipv6Addr::new(0x2a06, 0x98c0, 0, 0, 0, 0, 0, 0), 29),
    (Ipv6Addr::new(0x2c0f, 0xf248, 0, 0, 0, 0, 0, 0), 32),
];

fn ipv4_in_network(ip: u32, network: u32, prefix: u8) -> bool {
    let mask = if prefix >= 32 {
        u32::MAX
    } else {
        u32::MAX << (32 - prefix)
    };
    (ip & mask) == (network & mask)
}

fn ipv6_in_network(ip: u128, network: u128, prefix: u8) -> bool {
    let mask = if prefix >= 128 {
        u128::MAX
    } else {
        u128::MAX << (128 - prefix)
    };
    (ip & mask) == (network & mask)
}

pub(crate) fn is_cloudflare_peer(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let ip = u32::from(v4);
            CLOUDFLARE_IPV4
                .iter()
                .any(|(net, prefix)| ipv4_in_network(ip, u32::from(*net), *prefix))
        }
        IpAddr::V6(v6) => {
            let ip = u128::from(v6);
            CLOUDFLARE_IPV6
                .iter()
                .any(|(net, prefix)| ipv6_in_network(ip, u128::from(*net), *prefix))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cloudflare_peer_matches_known_range() {
        assert!(is_cloudflare_peer("104.16.42.1".parse().unwrap()));
        assert!(is_cloudflare_peer("172.64.0.1".parse().unwrap()));
        assert!(is_cloudflare_peer(
            "2606:4700:3037::6815:1234".parse().unwrap()
        ));
    }

    #[test]
    fn non_cloudflare_peer_rejected() {
        assert!(!is_cloudflare_peer("8.8.8.8".parse().unwrap()));
        assert!(!is_cloudflare_peer("203.0.113.7".parse().unwrap()));
        assert!(!is_cloudflare_peer("2001:db8::1".parse().unwrap()));
    }
}
