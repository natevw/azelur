# AzEl urrrrrrrrrrrrrr

rotctl proxy with firebending powers

## Usage

Builds a single binary. For now takes two arguments:

1. the local socket to listen on (e.g. `localhost:4533` or `0.0.0.0:4242`…)
2. the upstream (actual rotctld) to control (e.g. `192.168.1.42:4533`)

From this repo it can be started via e.g. `cargo run localhost:4533 192.168.42.209:4533`.

## What it does

Passes along *translated* [rotctl protocol](https://hamlib.sourceforge.net/html/rigctld.1.html#PROTOCOL) positioning commands to an upstream rotator.

Normally, a program like SatDump or Look4Sat or whatnot uses simple
`P nn.nnn nn.nnn` commands to change the position of the rotator directly.

This proxy adds an `X nn.nnn nn.nnn` command to set **adjustment** values
for the azimuth and elevation numbers respectively. This can be used to
"re-calibrate" an errant rotator without having to physically reposition.

For example, if GOES-18 should be at `P 203.9 34` but signal is actually peaking
best when the rotator is set to `P 172.5 37.3` then a correction can be made.
We want to subtract 31.4° from all our azimuth values, and add 3.3° to the elevation.


So:

1. start this proxy, connected to your rotator
2. use e.g. SatDump, but connected to your running proxy
3. open a second (manual) connection to the proxy to change the adjustment

No GUI will support the "calibration adjustment" since it's a made-up command here,
but I've been using [nc](https://packages.debian.org/stable/netcat-openbsd) to do it:

```
# first terminal window
cargo run 127.0.0.1:4533 192.168.42.209:4533

# second terminal window
nc localhost 4533
  X -31.4 3.3
```

You should get back a `RPRT 0` response to your `X …` command.
You can enter a single `x` on a line (or `\set_adj AZ EL` / `\get_adj`)
to read back the currently set offset.

This offset is shared between all connections to the proxy. So while SatDump
is tracking a satellite you can send `X -1 0` … `X -1.5 0` … `X -2.5 0` adjustments
via your `nc` sidechannel to change where the real (upstream) rotator point

Note that the calibration values are stored "globally" (between connections to the proxy) but not persistently (between multiple runs of new proxy instances);
they reset back to `X 0 0` each time you start running the proxy.

## Coming maybe

Next ideas for this:

* add algo for avoiding excessive cable wrap around mast
* passthrough of unknown commands to upstream
* better architecture/error handling
* realtime position estimates?
