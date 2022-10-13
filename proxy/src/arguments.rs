use clap::Parser;

#[derive(Parser, Debug)]
#[clap(name="ppaass-proxy", author="Qu Hao", version="0.1.0", about, long_about = None)]
/// The proxy side of the ppaass, which will proxy
/// the agent connection to the target server
pub(crate) struct ProxyServerArguments {
    /// Configuration file path
    #[clap(short = 'c', long, value_parser)]
    pub configuration_file: Option<String>,
    /// Port of the ppaass proxy
    #[clap(short = 'p', long, value_parser)]
    pub port: Option<u16>,
    /// The root directory used to store the rsa
    /// files for each user
    #[clap(long, value_parser)]
    pub rsa_dir: Option<String>,
    /// Whether enable compressing
    #[clap(long, value_parser)]
    pub compress: Option<bool>,
}
