# AnyEyeballs

This repository (along with https://github.com/MaxF12/anyeyeballs_orchestrator) is the source code required for running the AnyEyeballs test implementation.
Its goal is to utilize the widely adopted Happy Eyeballs system to give implicit load balancing control to servers. 

## Requirements
Rust version 1.51.0 or later as well as Socket2 version 0.3.19 and toml version 0.5.8 are required for running the client.
## Setup
Before running the client, the configuration in config.toml has to be updated. The configurable values are:

| Name             | Description                                                                                                                                                         |
|------------------|---------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| node             |                                                                                                                                                                     |
| ipv4             | The IPv4 address that the node will serve content on.                                                                                                               |
| ipv6             | The IPv6 address that the node will serve content on.                                                                                                               |
| port             | The port the node will serve content on.                                                                                                                            |
| connections      | The maximum number of allowed concurrent connections to be handled by this node.                                                                                    |
| sleep            | The time in seconds each thread will sleep to simulate the serving of a connection.                                                                                 |
| report\_interval | The time in seconds after which a new load status report will be sent to the LBM.                                                                                   |
| rtt\_threshold   | The load threshold which will measure as a reference point to measure the LBM response time. It should have the same value as the load\_threshold value on the LBM. |
| lbm              |                                                                                                                                                                     |
| ip               | The IP address the LBM uses to listen for incoming packets.                                                                                                         |
| port             | The port the LBM uses to listen for incoming packets.                                                                                                               |

## Running
To run the client, simply execute "cargo run config.toml" from the AnyEyeballs directory. 
