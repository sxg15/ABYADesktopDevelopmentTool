use super::models::LanInterface;
use crate::foundation::{AppError, AppResult};
use if_addrs::{IfAddr, get_if_addrs};
use serde_json::json;
use std::net::Ipv4Addr;

pub(crate) fn private_ipv4_interfaces() -> AppResult<Vec<LanInterface>> {
    let mut interfaces = get_if_addrs()
        .map_err(AppError::internal)?
        .into_iter()
        .filter_map(|interface| {
            let IfAddr::V4(address) = interface.addr else {
                return None;
            };
            is_private_adapter(address.ip).then(|| LanInterface {
                id: format!("{}@{}", interface.name, address.ip),
                name: interface.name,
                address: address.ip.to_string(),
                broadcast_address: broadcast_address(address.ip, address.netmask).to_string(),
            })
        })
        .collect::<Vec<_>>();
    interfaces.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then(left.address.cmp(&right.address))
    });
    Ok(interfaces)
}

pub(crate) fn preferred_interface<'a>(
    interfaces: &'a [LanInterface],
    preferred_id: &str,
) -> Option<&'a LanInterface> {
    interfaces
        .iter()
        .find(|interface| interface.id == preferred_id)
        .or_else(|| interfaces.first())
}

pub(crate) fn advertisement(tool_id: &str, display_name: &str, gateway_port: u16) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "magic": "ABYA_DEV_TOOL",
        "protocolVersion": super::models::PROTOCOL_VERSION,
        "toolId": tool_id,
        "displayName": display_name,
        "gatewayPort": gateway_port,
        "path": super::models::GATEWAY_PATH,
        "transport": "ws",
        "capabilities": ["logs", super::models::ARCHIVE_TRANSFER_CAPABILITY]
    }))
    .unwrap_or_default()
}

fn is_private_adapter(ip: Ipv4Addr) -> bool {
    ip.is_private() && !ip.is_loopback() && !ip.is_link_local()
}

fn broadcast_address(ip: Ipv4Addr, mask: Ipv4Addr) -> Ipv4Addr {
    let ip = u32::from(ip);
    let mask = u32::from(mask);
    Ipv4Addr::from((ip & mask) | !mask)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn calculates_subnet_broadcast_address() {
        assert_eq!(
            broadcast_address(
                Ipv4Addr::new(192, 168, 10, 12),
                Ipv4Addr::new(255, 255, 255, 0)
            ),
            Ipv4Addr::new(192, 168, 10, 255)
        );
    }

    #[test]
    fn selects_requested_adapter_then_falls_back() {
        let interfaces = vec![
            LanInterface {
                id: "a".into(),
                name: "A".into(),
                address: "192.168.1.2".into(),
                broadcast_address: "192.168.1.255".into(),
            },
            LanInterface {
                id: "b".into(),
                name: "B".into(),
                address: "10.0.0.2".into(),
                broadcast_address: "10.255.255.255".into(),
            },
        ];
        assert_eq!(preferred_interface(&interfaces, "b").unwrap().id, "b");
        assert_eq!(preferred_interface(&interfaces, "missing").unwrap().id, "a");
    }

    #[test]
    fn advertisement_uses_source_ip_instead_of_embedding_an_address() {
        let value: Value =
            serde_json::from_slice(&advertisement("tool", "Desktop", 47610)).unwrap();
        assert_eq!(value["magic"], "ABYA_DEV_TOOL");
        assert_eq!(value["protocolVersion"], 1);
        assert!(value.get("address").is_none());
        assert!(value.get("ip").is_none());
    }
}
