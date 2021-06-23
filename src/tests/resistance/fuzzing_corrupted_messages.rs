use crate::{
    tools::synthetic_peers::SyntheticNode,
    protocol::payload::{codec::Codec, Version},
    setup::node::{Action, Node},
    tests::resistance::{
        default_fuzz_messages, seeded_rng, Message, DISCONNECT_TIMEOUT, ITERATIONS,
    },
};

use std::sync::Arc;

use assert_matches::assert_matches;
use parking_lot::RwLock;
use rand::prelude::{Rng, SliceRandom};
use rand_chacha::ChaCha8Rng;

const CORRUPTION_PROBABILITY: f64 = 0.5;

#[tokio::test]
async fn corrupted_version_pre_handshake() {
    // ZG-RESISTANCE-001 (part 4)
    //
    // This particular case is considered alone because it is at particular risk of causing
    // troublesome behaviour, as seen with the valid metadata fuzzing against zebra.
    //
    // zebra: sends a version before disconnecting.
    // zcashd: ignores message but doesn't disconnect.
    // (log ex:
    // `INFO main: PROCESSMESSAGE: INVALID MESSAGESTART ;ersion peer=3`
    // `INFO main: ProcessMessages(version, 86 bytes): CHECKSUM ERROR nChecksum=67412de1 hdr.nChecksum=ddca6880`
    // which indicates the message was recognised as invalid).

    let mut rng = seeded_rng();

    let mut node: Node = Default::default();
    node.initial_action(Action::WaitForConnection).start().await;

    let synth_builder = SyntheticNode::builder().with_all_auto_reply();

    for _ in 0..ITERATIONS {
        let mut synth_node = synth_builder.build().await.unwrap();
        synth_node.connect(node.addr()).await.unwrap();

        let version = Message::Version(Version::new(node.addr(), synth_node.listening_addr()));
        let corrupted_version = corrupt_message(&mut rng, &version);

        // Send corrupt Version in place of Verack.
        // Contains header + message.
        synth_node
            .send_direct_bytes(node.addr(), corrupted_version)
            .await
            .unwrap();

        assert!(synth_node
            .wait_for_disconnect(node.addr(), DISCONNECT_TIMEOUT)
            .await
            .is_ok());
    }

    node.stop().await;
}

#[tokio::test]
async fn corrupted_version_during_handshake_responder_side() {
    // ZG-RESISTANCE-002 (part 4)
    //
    // This particular case is considered alone because it is at particular risk of causing
    // troublesome behaviour, as seen with the valid metadata fuzzing against zebra.
    //
    // zebra: sends a verack before disconnecting (though somewhat slow running).
    // zcashd: logs suggest the message was ignored but the node doesn't disconnect.

    let mut rng = seeded_rng();

    let mut node: Node = Default::default();
    node.initial_action(Action::WaitForConnection).start().await;

    let synth_builder = SyntheticNode::builder()
        .with_version_exchange_handshake()
        .with_all_auto_reply();

    for _ in 0..ITERATIONS {
        let mut synth_node = synth_builder.build().await.unwrap();
        synth_node.connect(node.addr()).await.unwrap();

        let version_to_corrupt =
            Message::Version(Version::new(node.addr(), synth_node.listening_addr()));
        let corrupted_version = corrupt_message(&mut rng, &version_to_corrupt);

        // Send corrupt Version in place of Verack.
        // Contains header + message.
        synth_node
            .send_direct_bytes(node.addr(), corrupted_version)
            .await
            .unwrap();

        assert!(synth_node
            .wait_for_disconnect(node.addr(), DISCONNECT_TIMEOUT)
            .await
            .is_ok());
    }

    node.stop().await;
}

#[tokio::test]
async fn corrupted_version_when_node_initiates_handshake() {
    // ZG-RESISTANCE-003 (part 4, version only)
    //
    // This particular case is considered alone because it is at particular risk of causing
    // troublesome behaviour, as seen with the valid metadata fuzzing against zebra.
    //
    // zebra: disconnects immediately.
    // zcashd: Some messages get ignored and timeout. Most cause an immedietely due to
    //          - main: PROCESSMESSAGE: INVALID MESSAGESTART, or
    //          - net: Oversized message from peer

    let rng = Arc::new(RwLock::new(seeded_rng()));

    // create peers (we need their ports to give to the node)
    let (synth_nodes, synth_addrs) = SyntheticNode::builder()
        .with_all_auto_reply()
        .build_n(ITERATIONS)
        .await
        .unwrap();

    // start peer processes
    let mut synth_handles = Vec::with_capacity(synth_nodes.len());
    for mut synth_node in synth_nodes {
        let rng = rng.clone();
        synth_handles.push(tokio::time::timeout(
            tokio::time::Duration::from_secs(120),
            tokio::spawn(async move {
                // Await connection and receive version
                synth_node.wait_for_connection().await;
                let (node_addr, version) = synth_node.recv_message().await;
                assert_matches!(version, Message::Version(..));

                // send bad version
                let corrupted_version = {
                    let mut rng = rng.write();
                    let version_to_corrupt =
                        Message::Version(Version::new(node_addr, synth_node.listening_addr()));
                    corrupt_message(&mut rng, &version_to_corrupt)
                };
                synth_node
                    .send_direct_bytes(node_addr, corrupted_version)
                    .await
                    .unwrap();

                assert!(synth_node
                    .wait_for_disconnect(node_addr, DISCONNECT_TIMEOUT)
                    .await
                    .is_ok());
            }),
        ));
    }

    let mut node: Node = Default::default();
    node.initial_action(Action::None)
        .initial_peers(synth_addrs)
        .start()
        .await;

    // join the peer processes
    for handle in synth_handles {
        handle.await.unwrap().unwrap();
    }

    node.stop().await;
}

#[tokio::test]
async fn corrupted_version_inplace_of_verack_when_node_initiates_handshake() {
    // ZG-RESISTANCE-004 (part 4, version only)
    //
    // This particular case is considered alone because it is at particular risk of causing
    // troublesome behaviour, as seen with the valid metadata fuzzing against zebra.
    //
    // zebra: disconnects immediately.
    // zcashd: Sends GetAddr, Ping and GetHeaders. Appears to ignore bad verack message.

    let locked_rng = Arc::new(RwLock::new(seeded_rng()));

    // create peers (we need their ports to give to the node)
    let (synth_nodes, synth_addrs) = SyntheticNode::builder()
        .with_version_exchange_handshake()
        .with_all_auto_reply()
        .build_n(ITERATIONS)
        .await
        .unwrap();

    // start peer processes
    let mut synth_handles = Vec::with_capacity(synth_nodes.len());
    for mut synth_node in synth_nodes {
        let rng = locked_rng.clone();
        synth_handles.push(tokio::time::timeout(
            tokio::time::Duration::from_secs(120),
            tokio::spawn(async move {
                // Await connection
                let node_addr = synth_node.wait_for_connection().await;

                // Receive verack
                let (_, verack) = synth_node.recv_message().await;
                assert_matches!(verack, Message::Verack);

                // Send bad version instead of verack
                let version =
                    Message::Version(Version::new(node_addr, synth_node.listening_addr()));
                let corrupted_version = {
                    let mut rng = rng.write();
                    corrupt_message(&mut rng, &version)
                };
                synth_node
                    .send_direct_bytes(node_addr, corrupted_version)
                    .await
                    .unwrap();

                assert!(synth_node
                    .wait_for_disconnect(node_addr, DISCONNECT_TIMEOUT)
                    .await
                    .is_ok());
            }),
        ));
    }

    let mut node: Node = Default::default();
    node.initial_action(Action::None)
        .initial_peers(synth_addrs)
        .start()
        .await;

    // join the peer processes
    for handle in synth_handles {
        handle.await.unwrap().unwrap();
    }

    node.stop().await;
}

#[tokio::test]
async fn corrupted_version_post_handshake() {
    // ZG-RESISTANCE-005 (part 4)
    //
    // This particular case is considered alone because it is at particular risk of causing
    // troublesome behaviour, as seen with the valid metadata fuzzing against zebra.
    //
    // zebra: disconnects
    // zcashd: disconnects for some messages, hangs for others.

    let mut rng = seeded_rng();
    let mut node: Node = Default::default();
    node.initial_action(Action::WaitForConnection).start().await;

    let synth_builder = SyntheticNode::builder()
        .with_all_auto_reply()
        .with_full_handshake();

    for _ in 0..ITERATIONS {
        let mut synth_node = synth_builder.build().await.unwrap();
        synth_node.connect(node.addr()).await.unwrap();

        let version_to_corrupt =
            Message::Version(Version::new(node.addr(), synth_node.listening_addr()));
        let corrupted_version = corrupt_message(&mut rng, &version_to_corrupt);

        // Send corrupt Version in place of Verack.
        // Contains header + message.
        synth_node
            .send_direct_bytes(node.addr(), corrupted_version)
            .await
            .unwrap();

        assert!(synth_node
            .wait_for_disconnect(node.addr(), DISCONNECT_TIMEOUT)
            .await
            .is_ok());
    }

    node.stop().await;
}

#[tokio::test]
async fn corrupted_messages_pre_handshake() {
    // ZG-RESISTANCE-001 (part 4)
    //
    // zebra: responds with a version before disconnecting (however, quite slow running).
    // zcashd: just ignores the message and doesn't disconnect.

    let test_messages = default_fuzz_messages();

    let mut rng = seeded_rng();
    let payloads = slightly_corrupted_messages(&mut rng, ITERATIONS, &test_messages);

    let mut node: Node = Default::default();
    node.initial_action(Action::WaitForConnection).start().await;

    let synth_builder = SyntheticNode::builder().with_all_auto_reply();

    for payload in payloads {
        let mut synth_node = synth_builder.build().await.unwrap();
        synth_node.connect(node.addr()).await.unwrap();

        synth_node
            .send_direct_bytes(node.addr(), payload)
            .await
            .unwrap();

        assert!(synth_node
            .wait_for_disconnect(node.addr(), DISCONNECT_TIMEOUT)
            .await
            .is_ok());
    }

    node.stop().await;
}

#[tokio::test]
async fn corrupted_messages_during_handshake_responder_side() {
    // ZG-RESISTANCE-002 (part 4)
    //
    // zebra: responds with verack before disconnecting (however, quite slow running).
    // zcashd: Some variants result in a terminated connect, some get ignored.

    let test_messages = default_fuzz_messages();

    let mut rng = seeded_rng();
    let payloads = slightly_corrupted_messages(&mut rng, ITERATIONS, &test_messages);

    let mut node: Node = Default::default();
    node.initial_action(Action::WaitForConnection).start().await;

    let synth_builder = SyntheticNode::builder()
        .with_version_exchange_handshake()
        .with_all_auto_reply();

    for payload in payloads {
        let mut synth_node = synth_builder.build().await.unwrap();
        synth_node.connect(node.addr()).await.unwrap();

        // Write the corrupted message in place of Verack.
        synth_node
            .send_direct_bytes(node.addr(), payload)
            .await
            .unwrap();

        assert!(synth_node
            .wait_for_disconnect(node.addr(), DISCONNECT_TIMEOUT)
            .await
            .is_ok());
    }

    node.stop().await;
}

#[tokio::test]
async fn corrupted_messages_inplace_of_version_when_node_initiates_handshake() {
    // ZG-RESISTANCE-003 (part 4)
    //
    // zebra: disconnects immediately.
    // zcashd: Some messages get ignored and timeout.
    //
    // Note: zcashd is two orders of magnitude slower (~52 vs ~0.5 seconds)

    let test_messages = default_fuzz_messages();

    let mut rng = seeded_rng();
    let mut payloads = slightly_corrupted_messages(&mut rng, ITERATIONS, &test_messages);

    // create peers (we need their ports to give to the node)
    let (synth_nodes, synth_addrs) = SyntheticNode::builder()
        .with_all_auto_reply()
        .build_n(ITERATIONS)
        .await
        .unwrap();

    // start peer processes
    let mut synth_handles = Vec::with_capacity(synth_nodes.len());
    for mut synth_node in synth_nodes {
        let payload = payloads.pop().unwrap();
        synth_handles.push(tokio::time::timeout(
            tokio::time::Duration::from_secs(120),
            tokio::spawn(async move {
                // Await connection and receive version
                let node_addr = synth_node.wait_for_connection().await;

                let (_, version) = synth_node.recv_message().await;
                assert_matches!(version, Message::Version(..));

                // send bad version
                synth_node
                    .send_direct_bytes(node_addr, payload)
                    .await
                    .unwrap();

                assert!(synth_node
                    .wait_for_disconnect(node_addr, DISCONNECT_TIMEOUT)
                    .await
                    .is_ok());
            }),
        ));
    }

    let mut node: Node = Default::default();
    node.initial_action(Action::None)
        .initial_peers(synth_addrs)
        .start()
        .await;

    // join the peer processes
    for handle in synth_handles {
        handle.await.unwrap().unwrap();
    }

    node.stop().await;
}

#[tokio::test]
async fn corrupted_messages_inplace_of_verack_when_node_initiates_handshake() {
    // ZG-RESISTANCE-004 (part 4)
    //
    // zebra: disconnects immediately.
    // zcashd: Some messages get ignored and timeout. Otherwise sends GetAddr, Ping and GetHeaders
    //         before disconnecting.
    //
    // Note: zcashd is two orders of magnitude slower (~52 vs ~0.5 seconds)

    let test_messages = default_fuzz_messages();

    let mut rng = seeded_rng();
    let mut payloads = slightly_corrupted_messages(&mut rng, ITERATIONS, &test_messages);

    // create peers (we need their ports to give to the node)
    let (synth_nodes, synth_addrs) = SyntheticNode::builder()
        .with_version_exchange_handshake()
        .with_all_auto_reply()
        .build_n(ITERATIONS)
        .await
        .unwrap();

    // start peer processes
    let mut synth_handles = Vec::with_capacity(synth_nodes.len());
    for mut synth_node in synth_nodes {
        let payload = payloads.pop().unwrap();
        synth_handles.push(tokio::time::timeout(
            tokio::time::Duration::from_secs(120),
            tokio::spawn(async move {
                // Await connection
                let node_addr = synth_node.wait_for_connection().await;

                // Receive verack
                let (_, verack) = synth_node.recv_message().await;
                assert_matches!(verack, Message::Verack);

                // send bad verack
                synth_node
                    .send_direct_bytes(node_addr, payload)
                    .await
                    .unwrap();

                assert!(synth_node
                    .wait_for_disconnect(node_addr, DISCONNECT_TIMEOUT)
                    .await
                    .is_ok());
            }),
        ));
    }

    let mut node: Node = Default::default();
    node.initial_action(Action::None)
        .initial_peers(synth_addrs)
        .start()
        .await;

    // join the peer processes
    for handle in synth_handles {
        handle.await.unwrap().unwrap();
    }

    node.stop().await;
}

#[tokio::test]
async fn corrupted_messages_post_handshake() {
    // ZG-RESISTANCE-005 (part 4)
    //
    // zebra: sends getdata and ignores message.
    // zcashd: disconnects for some messages, hangs for others.

    let test_messages = default_fuzz_messages();

    let mut rng = seeded_rng();
    let payloads = slightly_corrupted_messages(&mut rng, ITERATIONS, &test_messages);

    let mut node: Node = Default::default();
    node.initial_action(Action::WaitForConnection).start().await;

    let synth_builder = SyntheticNode::builder()
        .with_all_auto_reply()
        .with_full_handshake();

    for payload in payloads {
        let mut synth_node = synth_builder.build().await.unwrap();
        synth_node.connect(node.addr()).await.unwrap();

        // Write the corrupted message in place of Verack.
        synth_node
            .send_direct_bytes(node.addr(), payload)
            .await
            .unwrap();

        assert!(synth_node
            .wait_for_disconnect(node.addr(), DISCONNECT_TIMEOUT)
            .await
            .is_ok());
    }

    node.stop().await;
}

// Corrupt messages from the supplied set by replacing a random number of bytes with random bytes.
pub fn slightly_corrupted_messages(
    rng: &mut ChaCha8Rng,
    n: usize,
    messages: &[Message],
) -> Vec<Vec<u8>> {
    (0..n)
        .map(|_| {
            let message = messages.choose(rng).unwrap();
            corrupt_message(rng, &message)
        })
        .collect()
}

fn corrupt_message(rng: &mut ChaCha8Rng, message: &Message) -> Vec<u8> {
    let mut message_buffer = vec![];
    let header = message.encode(&mut message_buffer).unwrap();
    let mut header_buffer = vec![];
    header.encode(&mut header_buffer).unwrap();

    let mut corrupted_header = corrupt_bytes(rng, &header_buffer);
    let mut corrupted_message = corrupt_bytes(rng, &message_buffer);

    corrupted_header.append(&mut corrupted_message);

    // Contains header + message.
    corrupted_header
}

fn corrupt_bytes(rng: &mut ChaCha8Rng, serialized: &[u8]) -> Vec<u8> {
    serialized
        .iter()
        .map(|byte| {
            if rng.gen_bool(CORRUPTION_PROBABILITY) {
                rng.gen()
            } else {
                *byte
            }
        })
        .collect()
}
