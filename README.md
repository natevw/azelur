# AzEl urrrrrrrrrrrrrr

rotctl proxy with firebending powers

## What it does

Passes along *translated* [rotctl protocol](https://hamlib.sourceforge.net/html/rigctld.1.html#PROTOCOL) positioning commands to an upstream rotator.

Normally, a program like SatDump or Look4Sat or whatnot uses simple
`P nn.nnn nn.nnn` commands to change the position of the rotator directly.

This proxy adds an `X nn.nnn nn.nnn` command to set **adjustment** values
for the azimuth and elevation numbers respectively. This can be used to
"re-calibrate" an errant rotator without having to physically reposition.

## Usage

Builds a single binary. Takes two required arguments:

`azelur <listen_addr:port> <upstream_addr:port>`

1. the local socket to listen on (e.g. `localhost:4533` or `0.0.0.0:4242`…)
2. the upstream (actual rotctld) to control (e.g. `192.168.1.42:4533`)

From this repo it can be started via e.g. `cargo run localhost:4533 192.168.42.209:4533`.

Also supports some optional environment variables:

* `AZEL_INITIAL_ADJ` — a string containing any initial `"<az_adj> <el_adj>"` values you want to pre-populate
* `AZEL_INITIAL_POS` — by default azelur will query the upstream rotator, otherwise to avoid this you can provide an initial (virtual) position
* `AZEL_MAX_WINDING` — azelur will *attempt* to keep the rotator from winding the cables more than this amount (assuming the cables are most slack/neutral at the 0° position)
  * defaults to `360.0` i.e. a full rotation in *either* direction. anything less than 180° is counterproductive and will be rejected on startup.
  * when azelur detects that an incoming position will over-wind the upstream rotator, it sends the rotator to an intermediate position first (in the opposite direction) before continuing to unwind from there to the requested position
  * ⚠️ CAUTION: the current implementation makes certain assumptions about how/when the rotator responds to commands, which are only tentatively true on my own rotator (Antrunner Pro) and may be completely ineffective on your rotator. Consider this an **EXPERIMENTAL** feature and do not rely on it. Not even if it seems to initially work!

As a general example of passing options, here we provide an initial calibration but still let it query the upstream for its own initial (i.e. current) position:

```
AZEL_INITIAL_ADJ="-5 +1.5" azelur 0.0.0.0:4533 192.168.42.209:4533
```

## Commands

A very limited subset of the [rotctl protocol](https://hamlib.sourceforge.net/html/rotctl.1.html#COMMANDS) is supported:

* `\set_pos` aka `P <az> <el>`
* `\get_pos` aka `p`

Plus as our extended (non-standard) commands:

* `\set_adj` aka `X <az_adj> <el_adj>`
* `\get_adj` aka `x`
* `\want_pos` aka `W <az> <el>`

## General concept

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
cargo run 0.0.0.0:4533 192.168.42.209:4533

# second terminal window
nc localhost 4533
  X -31.4 3.3
```

You should get back a `RPRT 0` response to your `X …` command.
You can enter a single `x` on a line (or `\set_adj AZ EL` / `\get_adj`)
to read back the currently set offset.

There's also a "math helper" shortcut `W <az> <el>` which will calculate the adjustment for you. For example, if you're calibrating against a geostationary satellite:

1. Start looking for the satellite at `P 203.9 34`
2. Drive it around until you find it at e.g. `P 172.5 37.3`
3. Tell azelur you **W**ant the current position to be: `W 203.9 34`
4. Azelur will calculate and update the necessary adjustment
  * in this example `x` would return `-31.4`/`3.3` (if there had been no prior adjustment)

### Misc. details

This offset is shared between all connections to the proxy. So while SatDump
is tracking a satellite you can send `X -1 0` … `X -1.5 0` … `X -2.5 0` adjustments
via your `nc` sidechannel to change where the real (upstream) rotator point

Note that the calibration values are stored "globally" (between connections to the proxy) but not persistently (between multiple runs of new proxy instances);
they reset back to `X 0 0` each time you start running the proxy unless you set the `AZEL_INITIAL_ADJ` to a string containing the az_adj and el_adj values separated by a space e.g. `export AZEL_INITIAL_ADJ="-31.4 3.3"`.

## Testing notes

Mostly just a memory-jogger for my own local development:

```
nc -kl 4535  # fake upstream listener
AZEL_INITIAL_POS="0 0" cargo run 0.0.0.0:4533 localhost:4535
nc localhost 4533
  P 360 25
  P 365 0
```

### Container

For docker/podman/container can e.g.:

```
podman build -t azelur .

# NOTE: the default entrypoint listens on `0.0.0.0:4533`,
# so you only need to pass the upstream rotator address.
podman run --name azel-proxy --rm -d -p 4533:4533 azelur 192.168.42.209:4533
```

## Coming maybe

Next ideas for this:

* passthrough of unknown commands to upstream?
* better architecture/error handling
* realtime position estimates?
