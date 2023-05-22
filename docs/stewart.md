# Stewart

Stewart is a minimalist, high-performance, and non-exclusive actor system.

- Minimalist: Starts from a small self-contained and thread-local actor system with minimal
    assumptions. Everything else is built on top, including threading!
- High-Performance: Built around real-time rendering use cases. Fearlessly use stewart for
    anything!
- Non-Exclusive: Plays nicely with other actor systems, async runtimes, web-workers, GPU pipelines,
    distributed frameworks, etc... Stewart doesn't limit what you can interact with.

## Why another actor library?

While many actor libraries already exist in rust, they are generally made for web servers.
In web servers, performance and latency is often negligible compared to the cost of IO.

Web servers are also generally expected to run as a native binary on a typical dedicated instance.
Stewart doesn't make this assumption, and can be run in 'weird' contexts, like web workers.
