# FayRPG integration branch

This branch carries the reproducible client and open.mp server builds used by
FayRPG. It started from upstream commit
`f14fe4fa7f0d036edf445ba02b4bddd4cccac377`.

## Intentional differences

- The packaged client reads `port_offset` from `cef/config.json`.
- Linux open.mp artifacts are built and checked as 32-bit x86, matching the
  FayRPG server and its existing components.
- The client pins `bnece/samp-client-api` commit `b7d7a0e`, which resolves
  `samp.dll` from the loaded module instead of the launcher's current working
  directory. This prevents supported 0.3.DL clients started through SAMPFix
  from being reported as an unknown SA:MP version.
- Client and open.mp component default to a port offset of `100`. FayRPG game
  ports `7777` through `7781` therefore use CEF UDP ports `7877` through
  `7881`, avoiding collisions between server instances.
- The FayRPG workflow builds the Linux component on Ubuntu 22.04 against the
  open.mp `v1.5.8.3079` SDK used by the server image.
- Client packages contain the local `fayrpg-smoke/index.html` diagnostic page.

Do not commit generated client or server binaries. GitHub Actions artifacts
are the build products; consumers record the source commit, workflow run and
SHA-256 next to the installed binary.
