use clap::{Arg, Command};
use mantra_dex_sdk::mcp::server::{create_http_server, create_stdio_server, McpServerConfig};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();

    let matches = Command::new("Mantra DEX MCP Server")
        .version("0.1.0")
        .about("Model Context Protocol server for Mantra DEX operations")
        .arg(
            Arg::new("transport")
                .long("transport")
                .short('t')
                .value_name("TYPE")
                .help("Transport type: stdio or http")
                .default_value("stdio")
                .value_parser(["stdio", "http"]),
        )
        .arg(
            Arg::new("host")
                .long("host")
                .value_name("HOST")
                .help("Host to bind HTTP server to")
                .default_value("127.0.0.1"),
        )
        .arg(
            Arg::new("port")
                .long("port")
                .short('p')
                .value_name("PORT")
                .help("Port to bind HTTP server to")
                .default_value("8080")
                .value_parser(clap::value_parser!(u16)),
        )
        .arg(
            Arg::new("network")
                .long("network")
                .short('n')
                .value_name("NETWORK")
                .help("Network to connect to")
                .default_value("testnet")
                .value_parser(["mainnet", "testnet"]),
        )
        .arg(
            Arg::new("debug")
                .long("debug")
                .short('d')
                .help("Enable debug logging")
                .action(clap::ArgAction::SetTrue),
        )
        .get_matches();

    // Parse arguments
    let transport = matches.get_one::<String>("transport").unwrap();
    let host = matches.get_one::<String>("host").unwrap().clone();
    let port = *matches.get_one::<u16>("port").unwrap();
    let network = matches.get_one::<String>("network").unwrap();
    let debug = matches.get_flag("debug");

    // Create server config with environment variables and CLI overrides
    let mut config = match McpServerConfig::with_network(network) {
        Ok(config) => config,
        Err(e) => {
            eprintln!("Failed to load configuration: {}", e);
            std::process::exit(1);
        }
    };

    // Override debug setting from CLI if provided
    if debug {
        config.debug = debug;
    }

    // Override host and port from CLI if using HTTP transport
    if transport == "http" {
        config.http_host = host;
        config.http_port = port;
    }

    // Validate the final configuration
    if let Err(e) = config.validate() {
        eprintln!("Invalid configuration: {}", e);
        std::process::exit(1);
    }

    tracing::info!(
        "Starting Mantra DEX MCP Server on {} network with {} transport",
        network,
        transport
    );

    match transport.as_str() {
        "stdio" => {
            tracing::info!("Using stdio transport for MCP communication");
            create_stdio_server(config).await?;
        }
        "http" => {
            let http_host = config.http_host.clone();
            let http_port = config.http_port;
            tracing::info!("Using HTTP transport on {}:{}", http_host, http_port);
            create_http_server(config, http_host, http_port).await?;
        }
        _ => {
            eprintln!("Invalid transport type: {}", transport);
            std::process::exit(1);
        }
    }

    Ok(())
}
