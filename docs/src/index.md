# Introduction

Stewart is a lightweight actor system, with a modular ecosystem built around it.
It is built primarily for performance-sensitive and real-time applications like games, but can be
used anywhere.

At the core of stewart is the `stewart` crate, which implements a lightweight *single-threaded*
actor runtime.
All features are built on top of this core abstraction, including threading and distribution.

Stewart is meant to get out of your way as soon as possible, providing you just what you need to
develop reliable and high-performance asynchronous systems.

> 🚧 Stewart is very early in development.
> A lot of the features you may expect from a full-featured actor framework are not yet
> implemented.

## Who is this book for?

This book is a quick guide on stewart and its use of actors, to learn how and why to use it.
Additionally, this book gives advice on how to use stewart effectively in the real world.

The chapters are organized in *depth-first* intended reading order, when read as a complete guide.
