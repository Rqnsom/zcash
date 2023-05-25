//! A synthetic node binary can be used to interact with the node in the
//! background from a different runtime environment.
//!
//! This is a quick solution - in future Ziggurat projects it will be done differently.
//!
//! Remaining worklist (probably not going to be implemented in this repo):
//!   - Add an argument option to choose specific action in order to support
//!     different synthetic node binary implementations at once. A few examples:
//!     ```
//!        ./synthetic_node_bin --action=A    // Runs an idle/friendly synthetic node
//!        ./synthetic_node_bin --action=B    // Runs a wild synthetic node which does something funny
//!     ```
use std::{net::SocketAddr, process::ExitCode, str::FromStr};

use anyhow::Result;
use clap::Parser;
use tokio::time::{interval, sleep, Duration};
use ziggurat_zcash::{
    protocol::{
        message::Message,
        payload::{addr::NetworkAddr, Addr, Nonce},
    },
    tools::{
        message_filter::{Filter, MessageFilter},
        synthetic_node::SyntheticNode,
    },
};

/// A synthetic node which can connect to the XRPL node and preform some
/// actions independently.
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct CmdArgs {
    /// An address of the node in the <ip>:<port> format.
    #[arg(short = 'i', long)]
    node_addr: Option<SocketAddr>,

    /// Always reconnect in the case the connection fails - synthetic node never dies.
    #[arg(short = 's', long, default_value_t = false)]
    stubborn: bool,

    /// Enable tracing.
    #[arg(short = 't', long, default_value_t = false)]
    tracing: bool,
}

#[tokio::main]
async fn main() -> ExitCode {
    let args = CmdArgs::parse();

    let node_addr = if let Some(addr) = args.node_addr {
        addr
    } else {
        eprintln!("Node address should be provided.");
        return ExitCode::FAILURE;
    };

    if args.tracing {
        println!("Enabling tracing.");
        use tracing_subscriber::{fmt, EnvFilter};

        fmt()
            .with_test_writer()
            .with_env_filter(EnvFilter::from_default_env())
            .init();
    }

    loop {
        println!("Starting a synthetic node.");

        if let Err(e) = run_synth_node(node_addr).await {
            eprintln!("The synthetic node stopped: {e:?}.");
        }

        // Use the stubborn option to run the synth node infinitely.
        if !args.stubborn {
            break;
        }
    }

    ExitCode::SUCCESS
}

async fn run_synth_node(node_addr: SocketAddr) -> Result<()> {
    // Create a synthetic node and enable handshaking.
    let mut synth_node = SyntheticNode::builder()
        .with_full_handshake()
        .with_message_filter(msg_filter_cfg())
        .build()
        .await
        .unwrap();

    // Perform the handshake.
    synth_node.connect(node_addr).await?;

    // Run the wanted action with the node.
    perform_action(&mut synth_node, node_addr).await?;

    // Optional.
    sleep(Duration::from_millis(100)).await;

    // Stop the synthetic node.
    synth_node.shut_down().await;

    Ok(())
}

// --- TRY NOT TO CHANGE THE ABOVE CODE ---

// Configurable status printout interval.
const DBG_INFO_LOG_INTERVAL_SEC: Duration = Duration::from_secs(10);
const BROADCAST_INTERVAL_SEC: Duration = Duration::from_secs(120);

// Configurable message filter.
fn msg_filter_cfg() -> MessageFilter {
    MessageFilter::with_all_auto_reply().with_getaddr_filter(Filter::Disabled)
}

// Use this function to add some action which a synthetic node can do.
//
// All the program logic happens here.
#[allow(unused_variables, unused_must_use)]
async fn perform_action(synth_node: &mut SyntheticNode, addr: SocketAddr) -> Result<()> {
    println!("Synthetic node performs an action.");

    // Sleep for 3 seconds before taking any actions.
    sleep(Duration::from_millis(3000)).await;

    let msg = Message::GetAddr;
    tracing::info!("unicast {msg:?}\n");
    if synth_node.unicast(addr, msg.clone()).is_err() {
        tracing::warn!("failed to send {msg:?}\n");
        anyhow::bail!("connection closed");
    }

    let mut dbg_info_interval = interval(DBG_INFO_LOG_INTERVAL_SEC);
    let mut broadcast_msgs_interval = interval(BROADCAST_INTERVAL_SEC);

    loop {
        tokio::select! {
            _ = dbg_info_interval.tick() => {
                trace_debug_info(synth_node).await;
            },
            _ = broadcast_msgs_interval.tick() => {
                broadcast_periodic_msgs(synth_node);
            },
            Ok((src, msg)) = synth_node.try_recv_message() => {
                handle_rx_msg(synth_node, src, msg).await?;
            },
        }
    }
}

async fn trace_debug_info(synth_node: &SyntheticNode) {
    let peer_infos = synth_node.connected_peer_infos();
    let peer_cnt = peer_infos.len();

    let mut log = format!("\nNumber of peers: {peer_cnt}\n");

    // Let's sort by the connection's time value.
    let mut peer_infos: Vec<_> = peer_infos.iter().collect();
    peer_infos.sort_by(|a, b| a.1.stats().created().cmp(&b.1.stats().created()));

    for (addr, info) in peer_infos.iter() {
        let stats = info.stats();

        // Align all possible IP addresses (both v4 and v6) vertically
        const MAX_IPV6_ADDR_LEN: usize = 37;

        log.push_str(&format!(
            "{addr:>ident$}   ({side:?}) seconds: {time:?}\n",
            addr = addr,
            ident = MAX_IPV6_ADDR_LEN,
            side = info.side(),
            time = stats.created().elapsed()
        ));
    }

    tracing::info!("{log}");
}

fn broadcast_periodic_msgs(synth_node: &mut SyntheticNode) -> Result<()> {
    if let Err(e) = broadcast_ping_msg(synth_node) {
        tracing::warn!("failed to broadcast Ping messages: {e}");
    } else if let Err(e) = broadcast_addr_msg(synth_node) {
        tracing::warn!("failed to broadcast Addr messages {e}");
    } else if let Err(e) = broadcast_get_addr_msg(synth_node) {
        tracing::warn!("failed to broadcast GetAddr messages {e}");
    }

    Ok(())
}

fn broadcast_ping_msg(synth_node: &mut SyntheticNode) -> Result<()> {
    let msg = Message::Ping(Nonce::default());

    for addr in synth_node.connected_peers() {
        if synth_node.unicast(addr, msg.clone()).is_err() {
            tracing::error!("failed to send {msg:?} to {addr}\n");
            anyhow::bail!("connection closed");
        }
    }

    Ok(())
}

fn broadcast_get_addr_msg(synth_node: &mut SyntheticNode) -> Result<()> {
    let msg = Message::GetAddr;

    for addr in synth_node.connected_peers() {
        if synth_node.unicast(addr, msg.clone()).is_err() {
            tracing::error!("failed to send {msg:?} to {addr}\n");
            anyhow::bail!("connection closed");
        }
    }

    Ok(())
}

async fn handle_rx_msg(
    synth_node: &mut SyntheticNode,
    src: SocketAddr,
    msg: Message,
) -> Result<()> {
    tracing::info!("message received from {src}:\n{msg:?}");

    // We are only handling a GetAddr for now.
    if msg == Message::GetAddr {
        unicast_addr_msg(synth_node, src)?;
    }

    Ok(())
}

fn unicast_addr_msg(synth_node: &mut SyntheticNode, dst: SocketAddr) -> Result<()> {
    let addrs = vec![
        NetworkAddr::new(SocketAddr::from_str("35.210.227.177:8233").unwrap()),
        NetworkAddr::new(SocketAddr::from_str("35.205.233.245:8233").unwrap()),
        NetworkAddr::new(SocketAddr::from_str("35.205.233.245:46313").unwrap()),
    ];
    let msg = Message::Addr(Addr::new(addrs));

    tracing::info!("unicast {msg:?}\n");
    if synth_node.unicast(dst, msg.clone()).is_err() {
        tracing::warn!("failed to send {msg:?}\n");
        anyhow::bail!("connection closed");
    }

    Ok(())
}

fn broadcast_addr_msg(synth_node: &mut SyntheticNode) -> Result<()> {
    for addr in synth_node.connected_peers() {
        unicast_addr_msg(synth_node, addr)?;
    }

    Ok(())
}
