# VizBridge transport contract

## Internal DMX frame

A frame is transport-neutral:

```ts
type DmxFrame = {
  universe: number;       // 1-based UI universe
  sequence?: number;
  source: string;
  receivedAt: number;
  data: Uint8Array;       // exactly 512 slots
};
```

## Art-Net input

- UDP port: 6454
- Header: `Art-Net\0`
- OpCode: ArtDMX `0x5000` (little-endian)
- Protocol version: 14+
- Art-Net Port-Address is converted to VizBridge's 1-based universe at the adapter boundary.
- Declared DMX length must be even and between 2 and 512.
- Frames are normalized to 512 slots before entering the router.

## Rule

Network protocols terminate at adapters. The router and LumaViz must never depend on Art-Net packet structure.

## Diagnostics

Every received datagram is counted before parsing. VizBridge reports:
- UDP datagrams received
- accepted ArtDMX frames
- rejected datagrams + reason
- source IP/port
- universe
- sequence
- frames per second
- last receive time

This makes transport failure distinguishable from packet/parser failure.
