use std::{net::SocketAddr, time::Duration};

use assert_matches::assert_matches;

use crate::protocol::payload::block::{Headers, LocatorHashes};

use crate::{
    protocol::{
        message::Message,
        payload::{addr::NetworkAddr, Addr, Nonce},
    },
    setup::node::{Action, Node},
    tools::{
        message_filter::{Filter, MessageFilter},
        synthetic_node::SyntheticNode,
        LONG_TIMEOUT,
    },
    wait_until,
};

#[tokio::test]
async fn dev1_eagerly_crawls_network_for_peers() {
    // ZG-CONFORMANCE-013
    //
    // The node crawls the network for new peers and eagerly connects.
    //
    // Test procedure:
    //
    //  1. Create a set of peer nodes, listening concurrently
    //  2. Connect to node with another main peer node
    //  3. Wait for `GetAddr`
    //  4. Send set of peer listener node addresses
    //  5. Expect the node to connect to each peer in the set
    //
    // zcashd: Has different behaviour depending on connection direction.
    //         If we initiate the main connection it sends Ping, GetHeaders,
    //         but never GetAddr.
    //         If the node initiates then it does send GetAddr, but it never connects
    //         to the peers.
    //
    // zebra:  Fails, unless we keep responding on the main connection.
    //         If we do not keep responding then the peer connections take really long to establish,
    //         failing the test completely.
    //
    //         Nu5: fails, caches the addresses but doesn't open new connections, peer protocol
    //         tbc.
    //
    //         Related issues: https://github.com/ZcashFoundation/zebra/pull/2154
    //                         https://github.com/ZcashFoundation/zebra/issues/2163

    crate::tools::synthetic_node::enable_tracing();
    // Spin up a node instance.
    let mut node = Node::new().unwrap();
    node.initial_action(Action::WaitForConnection)
        .log_to_stdout(true)
        .start()
        .await
        .unwrap();

    //tokio::time::sleep(std::time::Duration::from_millis(5000)).await;

    // Create 5 synthetic nodes.
    const N: usize = 5;
    let (synthetic_nodes, addrs) = SyntheticNode::builder()
        .with_full_handshake()
        .with_all_auto_reply()
        .build_n(N)
        .await
        .unwrap();

    let addrs = addrs
        .iter()
        .map(|&addr| NetworkAddr::new(addr))
        .collect::<Vec<_>>();

    // Adjust the config so it lets through GetAddr message and start a "main" synthetic node which
    // will provide the peer list.
    let synthetic_node = SyntheticNode::builder()
        .with_full_handshake()
        .with_message_filter(
            MessageFilter::with_all_auto_reply().with_getaddr_filter(Filter::Disabled),
        )
        .build()
        .await
        .unwrap();

    // Connect and handshake.
    synthetic_node.connect(node.addr()).await.unwrap();

    // Expect GetAddr, this used to be necessary, as of Nu5, it may not be anymore.
    // let (_, getaddr) = synthetic_node.recv_message_timeout(TIMEOUT).await.unwrap();
    // assert_matches!(getaddr, Message::GetAddr);

    //let msg = Message::Addr(Addr::new(addrs));
    //tracing::info!("unicast {msg:?}\n");
    // Respond with peer list.
        //synthetic_node.unicast(node.addr(), msg).unwrap();
    //tracing::info!("unicasted\n");

    //tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    //synthetic_node.disconnect(node.addr()).await;
    //tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Connect and handshake.
    //synthetic_node.connect(node.addr()).await.unwrap();

    //let msg = Message::GetAddr;
//    tracing::info!("unicast {msg:?}\n");
    // Respond with peer list.
 //   synthetic_node
  //      .unicast(node.addr(), Message::Ping(Nonce::default()))
   //     .unwrap();
    //synthetic_node.unicast(node.addr(), msg.clone()).unwrap();
    //synthetic_node.unicast(node.addr(), msg.clone()).unwrap();
    //synthetic_node.unicast(node.addr(), msg.clone()).unwrap();
    //synthetic_node.unicast(node.addr(), msg.clone()).unwrap();

    tokio::time::sleep(std::time::Duration::from_secs(1000)).await;
    // Expect the synthetic nodes to get a connection request from the node.
    for node in synthetic_nodes {
        wait_until!(LONG_TIMEOUT, node.num_connected() == 1);

        node.shut_down().await;
    }

    // Gracefully shut down the node.
    node.stop().unwrap();
}

#[tokio::test]
async fn c013_eagerly_crawls_network_for_peers() {
    // ZG-CONFORMANCE-013
    //
    // The node crawls the network for new peers and eagerly connects.
    //
    // Test procedure:
    //
    //  1. Create a set of peer nodes, listening concurrently
    //  2. Connect to node with another main peer node
    //  3. Wait for `GetAddr`
    //  4. Send set of peer listener node addresses
    //  5. Expect the node to connect to each peer in the set
    //
    // zcashd: Has different behaviour depending on connection direction.
    //         If we initiate the main connection it sends Ping, GetHeaders,
    //         but never GetAddr.
    //         If the node initiates then it does send GetAddr, but it never connects
    //         to the peers.
    //
    // zebra:  Fails, unless we keep responding on the main connection.
    //         If we do not keep responding then the peer connections take really long to establish,
    //         failing the test completely.
    //
    //         Nu5: fails, caches the addresses but doesn't open new connections, peer protocol
    //         tbc.
    //
    //         Related issues: https://github.com/ZcashFoundation/zebra/pull/2154
    //                         https://github.com/ZcashFoundation/zebra/issues/2163

    crate::tools::synthetic_node::enable_tracing();
    // Spin up a node instance.

    //tokio::time::sleep(std::time::Duration::from_millis(5000)).await;

    // Adjust the config so it lets through GetAddr message and start a "main" synthetic node which
    // will provide the peer list.
    let mut synthetic_node = SyntheticNode::builder()
        .with_full_handshake()
        .with_message_filter(
            MessageFilter::with_all_auto_reply().with_getheaders_filter(Filter::Disabled),
        )
        .build()
        .await
        .unwrap();

    // Connect and handshake.
    //let addr = "127.0.0.1:8233".parse().unwrap();
    //
    // funny old guy from germany
    //let addr = "46.4.65.10:8233".parse().unwrap();

    //let addr = "149.56.29.232:8233".parse().unwrap(); // zebra! Montreal Canada -- less than 1000 nodes, cannot taint the peer list

    // some legend:
    // [does not connect (hs) - nothing -> we have a wrong port] -> nothing is running on this port, it the node is using some non-typical port
    // [timeout - no hs -> the node is full] -> yep, the node cannot accept new connections
    // 

    // ### ZCash most likely
    //let addr = "161.97.155.203:8233".parse().unwrap(); // 1000
    //let addr = "23.88.71.118:8233".parse().unwrap(); // 1000 a funny guy which returns all addresss in hte first rsp, but a single address later on
    //let addr = "95.216.115.54:8233".parse().unwrap(); // chapter done --- 1000
    //let addr = "84.75.28.247:8233".parse().unwrap(); // 1000 - switzerland
    //let addr = "185.244.237.130:8233".parse().unwrap(); // 1000 - france - east
    //let addr = "62.210.69.194:8233".parse().unwrap(); // 1000 - france paris
    let addr = "88.80.148.28:8233".parse().unwrap(); // 1000 - bulgaria
    //let addr = "78.189.206.225:8233".parse().unwrap(); // 1000 - turkey
    //let addr = "87.205.8.57:8233".parse().unwrap(); // poland krakow - does not connect (hs) - nothing -> we have a wrong port
    //let addr = "194.135.81.61:8233".parse().unwrap(); // lithuania - no hs - reject
    //let addr = "51.195.234.88:8233".parse().unwrap(); // london - connection refused
    //let addr = "51.222.152.238:8233".parse().unwrap(); // london - connection refused
    //let addr = "47.198.223.60:8233".parse().unwrap(); // 1000 florida
    //let addr = "35.206.143.131:8233".parse().unwrap(); // belgium - does not connect (hs) - nothing -> we have a wrong port
    //let addr = "138.201.252.11:8233".parse().unwrap(); // 1000 - berlin
    //let addr = "111.90.145.162:8233".parse().unwrap(); // 1000 - kuala lumpur
    //let addr = "52.28.203.21:8233".parse().unwrap(); // 1000 - frankfurt
    //let addr = "3.72.134.66:8233".parse().unwrap(); // 1000 - frankfurt
    //let addr = "18.193.245.53:8233".parse().unwrap(); // 1000 - frankfurt
    //let addr = "51.195.62.151:8233".parse().unwrap(); // 1000 - Limburg an der Lahn
    //let addr = "51.77.64.61:8233".parse().unwrap(); // 1000 - Limburg an der Lahn
    //let addr = "51.77.64.51:8233".parse().unwrap(); / 1000 - Limburg an der Lahn/
    //let addr = "5.2.75.10:8233".parse().unwrap(); // 1000 (latest) --- old: Netherlands Alkmaar - does not connect (hs) - nothing - hs timeout
    //let addr = "20.47.97.70:8233".parse().unwrap(); // 1000 - Amsterdam
    //let addr = "52.208.163.151:8233".parse().unwrap(); // Dublin - does not connect (hs) - nothing -> we have a wrong port
    //let addr = "54.194.53.104:8233".parse().unwrap(); // Dublin - does not connect (hs) - nothing -> we have a wrong port
    //let addr = "161.97.247.6:8233".parse().unwrap(); // USA Longmont - connection refused
    //let addr = "81.24.244.123:8233".parse().unwrap(); // Apatin - Serbia timeout - no hs -> the node is full
    //let addr = "50.220.121.211:8233".parse().unwrap(); // USA Eugene - does not connect (hs) - nothing -> we have a wrong port
    //let addr = "35.223.224.178:8233".parse().unwrap(); // USA The Dallas - does not connect (hs) - nothing -> we have a wrong port
    //let addr = "51.81.184.90:8233".parse().unwrap(); // USA Hillsboro - does not connect (hs) - nothing -> we have a wrong port
    //let addr = "51.81.184.89:8233".parse().unwrap(); // 1000 USA Hillsboro
    //let addr = "35.210.227.177:8233".parse().unwrap(); // ZIGGURAT - timeout - no hs -> the node is full
    //let addr = "192.166.217.71:8233".parse().unwrap(); // Piotr's zebrad
    //let addr = "[2001:41d0:203:8fc9::]:8836".parse().unwrap(); // unreachable??
    //let addr = "[2001:9e8:430b:7600:334f:12f:14ab:735c]:8233".parse().unwrap(); // unreachable??
    //let addr = "[2001:9e8:432a:6f00:6afa:dd7f:19e6:a70a]:8233".parse().unwrap(); // unreachable??
    //let addr = "[2620:a6:2000:1:1:0:3:dd15]:8233".parse().unwrap(); // unreachable??

    // ### non typical ports from other zcashd nodes
    //let addr = "51.195.63.10:8233".parse().unwrap(); // reported by another zcashd, nothing found here
    // #### the above one is an exception in this list - see #2
    //let addr = "141.95.45.187:30834".parse().unwrap(); // zcashd 1000 - nothing on 8233
    //let addr = "35.211.128.225:8333".parse().unwrap(); // zcashd 1000 - nothing on 8233
    // #### below ones were tested a day later after I saved their IP addresses
    //let addr = "223.72.35.142:2331".parse().unwrap(); // nothing
    //let addr = "124.126.140.29:2331".parse().unwrap(); // nothing
    //let addr = "223.72.35.182:2331".parse().unwrap(); // refused
    //let addr = "124.126.140.204:2331".parse().unwrap(); // nothing
    //let addr = "223.72.35.142:2331".parse().unwrap(); // nothing

    // ### non typical ports from Piotr's zebrad:
    //let addr = "51.195.63.10:30834".parse().unwrap(); // zcashd 1000 - nothing on 8233 - see #2
    //let addr = "51.210.220.135:8836".parse().unwrap(); // zcashd 1000 - reject on 8233
    //let addr = "35.211.104.188:8333".parse().unwrap(); // zcashd 1000 - nothing on 8233
    //let addr = "51.210.216.76:8836".parse().unwrap(); // zcashd 1000 - refused on 8233
    //let addr = "88.198.24.4:8533".parse().unwrap(); // zcashd 1000 - "YODA" on 8233, what is going on this IP??? see #1
    //let addr = "51.81.154.19:30834".parse().unwrap(); // zcashd 1000 - nothing on 8233
    //let addr = "123.114.102.54:2331".parse().unwrap(); // zcashd 1000 - refused on 8233
    //let addr = "64.201.115.143:54324".parse().unwrap(); // zcashd 1000 - refused on 8233
    //let addr = "35.211.164.183:8333".parse().unwrap(); // zcashd 1000 - nothing on 8233

    // ### YODA quick investigation #1
    //
    // all yoda peers are using the 1989 port
    // the original yoda requires a modified protocol version to 770007 or greater:
    //let addr = "88.198.24.4:8233".parse().unwrap(); // yoda 315 - nothing on 8233
    //
    // his peers:
    //let addr = "93.141.233.135:1989".parse().unwrap(); // Nothing
    //let addr = "194.163.152.124:1989".parse().unwrap(); // Nothing
    //let addr = "185.193.64.242:1989".parse().unwrap(); // Nothing
    //let addr = "37.120.207.190:1989".parse().unwrap(); // Nothing
    //let addr = "37.187.226.146:1989".parse().unwrap(); // Refused
    //let addr = "188.163.88.41:1989".parse().unwrap(); // Nothing
    //let addr = "40.68.221.133:1899".parse().unwrap(); // Refused
    //
    // What is this thing? ...whatever haha

    tracing::info!("pre-connect");
    synthetic_node.connect(addr).await.unwrap();
    tracing::info!("post-connect");

    // Expect GetAddr, this used to be necessary, as of Nu5, it may not be anymore.
    if false {
        use tokio::time::{sleep, Duration};
        const EXPECTED_RESULT_TIMEOUT: Duration = Duration::from_secs(30900);
        tokio::time::timeout(EXPECTED_RESULT_TIMEOUT, async {
            tracing::info!("Intrali");
            loop {
                let (_, msg) = synthetic_node.recv_message().await;
                match msg {
                    Message::GetHeaders(_) => {
                        tracing::info!("di su pare");
                        synthetic_node.unicast(addr, Message::Headers(Headers::empty())).unwrap();
                        tracing::info!("ma mozemo");
                        break;
                    },
                    _ => continue,
                }
            }
        })
        .await
        .unwrap();
    }

    tracing::info!("izosli");

    let send_msg = |msg| {
        tracing::info!("unicast {msg:?}\n");
        synthetic_node.unicast(addr, msg).unwrap();
    };

    //let msg = Message::Addr(Addr::new(addrs));
    //synthetic_node.unicast(addr, msg).unwrap();

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    //synthetic_node.disconnect(addr).await;
    tokio::time::sleep(std::time::Duration::from_millis(5000)).await;

    // Connect and handshake.
    //synthetic_node.connect(addr).await.unwrap();

    send_msg(Message::Ping(Nonce::default()));

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    send_msg(Message::GetAddr);

    //let stop_hash =
        //stop_hash.map_or_else(Hash::zeroed, |i| SEED_BLOCKS[i].double_sha256().unwrap());
    //let block_locator_hashes = vec![SEED_BLOCKS[locator_index].double_sha256().unwrap()];
    //let msg = Message::GetHeaders(LocatorHashes::new( block_locator_hashes, stop_hash)));

    //let msg = Message::GetHeaders(LocatorHashes::empty());
    //send_msg(msg);

    loop {
        let (_, msg) = synthetic_node.recv_message().await;
        match msg {
            Message::Addr(addr) => {
                tracing::info!("addr: {}", addr.addrs.len());
            },
            _ => continue,
        }
    }

    tokio::time::sleep(std::time::Duration::from_millis(80_000)).await;
}

#[tokio::test]
async fn c014_correctly_lists_peers() {
    // ZG-CONFORMANCE-014
    //
    // The node responds to a `GetAddr` with a list of peers it’s connected to. This command
    // should only be sent once, and by the node initiating the connection.
    //
    // In addition, this test case exercises the known zebra bug: https://github.com/ZcashFoundation/zebra/pull/2120
    //
    // Test procedure
    //      1. Establish N peer listeners
    //      2. Start node which connects to these N peers
    //      3. Create i..M new connections which,
    //          a) Connect to the node
    //          b) Query GetAddr
    //          c) Receive Addr == N peer addresses
    //
    // This test currently fails for both zcashd and zebra.
    //
    // Current behaviour:
    //
    //  zcashd: Never responds. Logs indicate `Unknown command "getaddr" from peer=1` if we initiate
    //          the connection. If the node initiates the connection then the command is recoginized,
    //          but likely ignored (because only the initiating node is supposed to send it).
    //
    //  zebra:  Never responds: "zebrad::components::inbound: ignoring `Peers` request from remote peer during network setup"
    //
    //          Nu5: never responds, not sure why, adding a timeout after the node start removes the setup error.
    //
    //          Can be coaxed into responding by sending a non-empty Addr in
    //          response to node's GetAddr. This still fails as it includes previous inbound
    //          connections in its address book (as in the bug listed above).

    // Create 5 synthetic nodes.
    const N: usize = 5;
    let node_builder = SyntheticNode::builder()
        .with_full_handshake()
        .with_all_auto_reply();
    let (synthetic_nodes, expected_addrs) = node_builder.build_n(N).await.unwrap();

    // Start node with the synthetic nodes as initial peers.
    let mut node = Node::new().unwrap();
    node.initial_action(Action::WaitForConnection)
        .initial_peers(expected_addrs.clone())
        .start()
        .await
        .unwrap();

    // This fixes the "setup incomplete" issue.
    // tokio::time::sleep(std::time::Duration::from_secs(10)).await;

    // Connect to node and request GetAddr. We perform multiple iterations to exercise the #2120
    // zebra bug.
    for _ in 0..N {
        let mut synthetic_node = node_builder.build().await.unwrap();

        synthetic_node.connect(node.addr()).await.unwrap();
        synthetic_node
            .unicast(node.addr(), Message::GetAddr)
            .unwrap();

        let (_, addr) = synthetic_node
            .recv_message_timeout(LONG_TIMEOUT)
            .await
            .unwrap();
        let addrs = assert_matches!(addr, Message::Addr(addrs) => addrs);

        // Check that ephemeral connections were not gossiped.
        let addrs: Vec<SocketAddr> = addrs.iter().map(|network_addr| network_addr.addr).collect();
        assert_eq!(addrs, expected_addrs);

        synthetic_node.shut_down().await;
    }

    // Gracefully shut down nodes.
    for synthetic_node in synthetic_nodes {
        synthetic_node.shut_down().await;
    }

    node.stop().unwrap();
}
