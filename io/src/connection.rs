use tokio::net::TcpStream;

pub struct PpaassProtocolConnection {
    tcp_stream: TcpStream,
}
