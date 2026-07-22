# Demo

Three ways to see Murmuration route a message across a mesh, fastest first.

## 1. Docker — a 3-node mesh in one command

No toolchain needed; just Docker.

```bash
docker compose up --build          # brings up node1 ← node2 ← node3 (a chain)
```

Each node runs an HTTP gateway: node1 on `:8001`, node2 on `:8002`, node3 on
`:8003`. The nodes form a chain, so content published on node1 has to be *routed*
to reach node3.

In another terminal, publish on node1 and fetch across the mesh from node3:

```bash
# publish a page on node1
curl -X POST http://localhost:8001/publish \
     -d 'path=hello.html' --data-urlencode 'body=<h1>Hello from the mesh</h1>'

# node1 prints its node id in the compose logs; call it $N1, then from node3:
curl "http://localhost:8003/ely/$N1/hello.html"
```

The fetch only succeeds because node2 relayed the request from node3 to node1 and
the response back — that is the mesh routing working. `Ctrl-C` then
`docker compose down` to stop.

## 2. Local binary — 3 nodes on one machine

Needs Rust. Builds and runs the real `mur` binary.

```bash
cargo install --path core --bin mur     # or: make install

# terminal 1 (bootstrap)
mur start 8080 --gateway 8000
# terminal 2
mur start 8081 127.0.0.1:8080 --gateway 8001
# terminal 3
mur start 8082 127.0.0.1:8081 --gateway 8002
```

Then publish/fetch/broadcast with the `mur` CLI — full walkthrough in
[../docs/DEMO.md](../docs/DEMO.md).

## 3. The routing study — no network needed

Reproduce the research result (the reason this project is interesting):

```bash
cargo run --release --bin benchmark      # static-graph: oracle bound, UCB1, Q-routing
cargo run --release --bin trace_bench     # delay-tolerant, contact-trace mobility
python3 results/make_figures.py           # regenerate the 7 figures
```

Read the write-up in [../results/RESULTS.md](../results/RESULTS.md).
