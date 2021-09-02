# AnyEyeballs

This repository (along with https://github.com/MaxF12/anyeyeballs_orchestrator) is the source code required for running the AnyEyeballs test application.
Its goal is to utilize the widely adopted HappyEyeballs protocol for client side load balancing. 

## Requirements
Rust version 1.51.0 or later as well as Socket2 version 0.3.19 are required for running the client.
## Setup
Before running the client, the local IPv4 and IPv6 address have to be configured in lines 12 and 13 of main.rs. Furthermore, the address on which the orchestrator will listen has to be required in line 13.
## Running
To run the client, simply execute cargo run from the AnyEyeballs directory. 
