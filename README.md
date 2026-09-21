# VizBridge

A focused DMX transport and diagnostics bridge for LumaViz.

## Purpose

VizBridge does one job: receive, inspect, generate, and forward DMX universe data between lighting controllers and visualizers.

It contains **no fixture definitions, show programming, effects, cues, or rendering logic**.

## v0.1 architecture

```
Controller / Test Generator
          |
          v
   Input Adapters
  Art-Net | Local
          |
          v
     DMX Router
  universe -> 512 bytes
          |
     +----+----+
     |         |
 Diagnostics  Output Adapters
               Art-Net | LumaViz local
```

### Initial milestone

1. Art-Net ArtDMX receive on UDP 6454.
2. Live packet/FPS/source/universe diagnostics.
3. 512-channel universe inspector.
4. Built-in DMX test generator.
5. Art-Net forwarding with explicit destination and universe mapping.
6. Local LumaViz transport after the DMX core is proven.

LumaRig remains out of this milestone. We first prove VizBridge and LumaViz independently.
