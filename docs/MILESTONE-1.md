# Milestone 1 — prove the DMX path

## Gate A — internal generator
Generate Universe 1 / Channel 1 values 0–255 without networking. Inspector must show the exact value.

## Gate B — Art-Net loopback
Generator sends a valid ArtDMX frame to 127.0.0.1:6454. Receiver must count the raw datagram and accepted frame independently.

## Gate C — forwarding
Receive a frame, route it, and forward it to a configured destination. Sent frame count and last destination must be visible.

## Gate D — LumaViz
LumaViz receives a transport-neutral DMX frame and applies it to a patched test fixture.

## Gate E — third-party source
Use an external Art-Net source before LumaRig. This proves the bridge and visualizer without involving LumaRig.

Only after A–E pass do we connect LumaRig.
