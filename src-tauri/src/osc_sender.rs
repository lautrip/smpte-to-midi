use rosc::{OscMessage, OscPacket, OscType};
use std::net::{SocketAddr, UdpSocket};

pub fn send_osc(ip_port: &str, address: &str, args: Vec<OscType>) -> Result<(), String> {
    let socket = UdpSocket::bind("0.0.0.0:0").map_err(|e| e.to_string())?;
    let target_addr: SocketAddr = ip_port
        .parse()
        .map_err(|e| format!("Invalid IP/Port: {}", e))?;

    let msg = OscPacket::Message(OscMessage {
        addr: address.to_string(),
        args,
    });

    let msg_buf = rosc::encoder::encode(&msg).map_err(|e| e.to_string())?;
    socket
        .send_to(&msg_buf, target_addr)
        .map_err(|e| e.to_string())?;

    Ok(())
}
