use crate::setup::config::{NodeConfig, NodeKind, NodeMetaData, ZcashdConfigFile, ZebraConfigFile};

use tokio::{
    net::TcpListener,
    process::{Child, Command},
};

use std::{fs, net::SocketAddr, process::Stdio};

/// Represents an instance of a node, its configuration and setup/teardown intricacies.
pub struct Node {
    /// The (external) address of a node.
    ///
    /// Nodes can have a local address distinct from the external address at
    /// which it is reachable (e.g. docker).
    addr: SocketAddr,
    /// Configuration definable in tests and written to the node's configuration file on start.
    config: NodeConfig,
    /// Type, path to binary, various commands for starting, stopping, cleanup, network
    /// configuration.
    meta: NodeMetaData,
    /// Process of the running node.
    process: Option<Child>,
}

impl Node {
    /// Creates a new [`Node`] instance from [`NodeMetaData`].
    ///
    /// Once created, it can be configured with calls to [`initial_peers`], [`max_peers`] and [`log_to_stdout`].
    ///
    /// [`Node`]: struct@Node
    /// [`NodeMetaData`]: struct@crate::setup::config::NodeMetaData
    /// [`initial_peers`]: methode@Node::initial_peers
    /// [`max_peers`]: methode@Node::max_peers
    /// [`log_to_stdout`]: method@Node::log_to_stdout
    pub fn new(meta: NodeMetaData) -> Self {
        // Config (to be written to node configuration file) sets the configured `local_ip`.
        let config = NodeConfig::new(meta.local_addr);

        // The node instance gets a node address which may differ from the `local_ip`.
        Self {
            // TODO: select random port to support multiple nodes.
            addr: meta.external_addr,
            config,
            meta,
            process: None,
        }
    }

    /// Returns the (external) address of the node.
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// Sets the initial peers (ports only) for the node.
    ///
    /// The ip used to construct the addresses can be optionally set in the configuration file and
    /// otherwise defaults to localhost.
    pub fn initial_peers(&mut self, peers: Vec<u16>) -> &mut Self {
        self.config.initial_peers = peers
            .iter()
            .map(|port| format!("{}:{}", self.meta.peer_ip, port))
            .collect();

        self
    }

    /// Sets the maximum connection value for the node.
    pub fn max_peers(&mut self, max_peers: usize) -> &mut Self {
        self.config.max_peers = max_peers;
        self
    }

    /// Sets whether to log the node's output to Ziggurat's output stream.
    pub fn log_to_stdout(&mut self, log_to_stdout: bool) -> &mut Self {
        self.config.log_to_stdout = log_to_stdout;
        self
    }

    /// Sets whether to signal the node has started through a peer connection.
    ///
    /// If set the call to [`start`] will initiate a listener set as a peer on the node and
    /// will only return once it has received a connection request. This isn't necessary in
    /// scenarios in which the node initates the connections.
    ///
    /// [`start`]: methode@Node::start
    pub fn start_waits_for_connection(&mut self, addr: SocketAddr) -> &mut Self {
        self.config.start_listener_addr = Some(addr);
        self
    }

    /// Starts the node instance.
    ///
    /// This function will write the appropriate configuration file and run the start command
    /// provided in `config.toml`.
    pub async fn start(&mut self) {
        // cleanup any previous runs (node.stop won't always be reached e.g. test panics, or SIGINT)
        self.cleanup();

        // Set the listener if start signalling is enabled.
        let mut listener: Option<TcpListener> = None;
        if let Some(addr) = self.config.start_listener_addr {
            let bound_listener = TcpListener::bind(addr).await.unwrap();

            self.config.initial_peers.insert(format!(
                "{}:{}",
                self.meta.peer_ip,
                bound_listener.local_addr().unwrap().port()
            ));

            listener = Some(bound_listener);
        }

        // Generate config files for Zebra or Zcashd node.
        self.generate_config_file();

        let (stdout, stderr) = match self.config.log_to_stdout {
            true => (Stdio::inherit(), Stdio::inherit()),
            false => (Stdio::null(), Stdio::null()),
        };

        let process = Command::new(&self.meta.start_command)
            .current_dir(&self.meta.path)
            .args(&self.meta.start_args)
            .stdin(Stdio::null())
            .stdout(stdout)
            .stderr(stderr)
            .kill_on_drop(true)
            .spawn()
            .expect("node failed to start");

        self.process = Some(process);

        // if start signal is expected, await connection before returning.
        if let Some(listener) = listener {
            listener.accept().await.unwrap();
        }
    }

    /// Stops the node instance.
    ///
    /// The stop command will only be run if provided in the `config.toml` file as it may not be
    /// necessary to shutdown a node (killing the process is sometimes sufficient).
    pub async fn stop(&mut self) {
        let mut child = self.process.take().unwrap();
        let stdout = match self.config.log_to_stdout {
            true => Stdio::inherit(),
            false => Stdio::null(),
        };

        // Simply kill the process if no stop command is provided. If it is, run it under the
        // assumption the process has already exited.
        match (
            self.meta.stop_command.as_ref(),
            self.meta.stop_args.as_ref(),
        ) {
            (Some(stop_command), Some(stop_args)) => {
                Command::new(stop_command)
                    .current_dir(&self.meta.path)
                    .args(stop_args)
                    .stdin(Stdio::null())
                    .stdout(stdout)
                    .status()
                    .await
                    .expect("failed to run stop command");
            }
            _ => child.kill().await.expect("failed to kill process"),
        }

        self.cleanup();
    }

    fn generate_config_file(&self) {
        let path = self.config_filepath();
        let content = match self.meta.kind {
            NodeKind::Zebra => ZebraConfigFile::generate(&self.config),
            NodeKind::Zcashd => ZcashdConfigFile::generate(&self.config),
        };

        fs::write(path, content).unwrap();
    }

    fn config_filepath(&self) -> std::path::PathBuf {
        match self.meta.kind {
            NodeKind::Zebra => self.meta.path.join("node.toml"),
            NodeKind::Zcashd => self.meta.path.join("zcash.conf"),
        }
    }

    fn cleanup(&self) {
        self.cleanup_config_file();
        self.cleanup_cache();
    }

    fn cleanup_config_file(&self) {
        let path = self.config_filepath();
        match std::fs::remove_file(path) {
            // File may not exist, so we let that error through
            Err(err) if err.kind() != std::io::ErrorKind::NotFound => {
                panic!("Error removing config file: {}", err)
            }
            _ => {}
        }
    }

    fn cleanup_cache(&self) {
        // No cache for zebra as it is configured in ephemeral mode
        if let NodeKind::Zcashd = self.meta.kind {
            // Default cache location is ~/.zcash
            let path = home::home_dir().unwrap().join(".zcash");

            match std::fs::remove_dir_all(path) {
                // Directory may not exist, so we let that error through
                Err(err) if err.kind() != std::io::ErrorKind::NotFound => {
                    panic!("Error cleaning up zcashd cache: {}", err)
                }
                _ => {}
            }
        }
    }
}
