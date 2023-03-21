# Distributed Key-Value Store

This repository contains the final project for course ID2203 in Distributed Systems - Advance Course at KTH, 2023.

## Usage

To run the code, it is assumed that both `rust` and `cargo` are installed locally, on a macOS/Unix operating system.

To compile the code, run:
- `cargo build`

To start up the servers, run the following three scripts:
- `sh run1.sh`
- `sh run2.sh`
- `sh run3.sh`

To run the client server (which can send requests to the servers), run the following:
- `sh runc.sh`

There is also a management client to interact with the servers, in addition to simulating certain edge-case scenarios (used for testing). To run the management client, run the following script:
- `sh runm.sh`

## API

When you have run the scripts specified above, you can interact with the servers using the following commands. For the client, the following operations are supported:
- `<NODE> <OP> <ARGS>`

Where `NODE` can be one of the server node ID's (1, 2 and 3 in the init scripts above). `OP` and `ARGS` can be one of the following:
- `read <KEY>`
- `write <KEY> <VALUE>`
- (?) `delete <KEY>`

An example sequence of commands could be:
- `1 write 35 hello`
- `2 read 35`
- `3 write 55 1234`
- `1 read 55`

For the management client, we have a similar format:
- `<NODE> <OP> <ARGS>`

Where `OP` and `ARGS` can be the following:
- `get_links 0` - retrieve broken links for a node (0 can be any number, not used but needed for parser)
- `break_link <OTHER_NODE>` - break the connection to the specified node (partial connectivity testing)

## Feature Breakdown

Here's a checklist for what features and functionality we'd like to implement in the project.
- [x] Read/write keys/values
- [ ] CAS Write/read?
- [ ] Delete values
- [x] Read client state (management client, retrieve broken links/break links)
- [x] Simulate partial connectivity (Omission)
- [ ] Crash recovery
- [ ] Error tolerance in all clients/servers (dont crash when failing to read received messages)

For testing, we'd like to have the following:
- [ ] Sequentially consistent reads/writes
- [ ] Linearizable reads/writes (Wing & Gong Algorithm?)
- [ ] Partial connectivity (read/writes still working)