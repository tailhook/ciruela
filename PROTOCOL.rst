=================
Ciruela Protocols
=================

There are three protocols used by ciruela:

1. HTTP just to show you some status pages
2. A protocol to send file data on top of TCP
3. A gossip protocol on top of UDP

In future we will probably expose some JSON API over HTTP just to allow easier
interoperability with the daemon.


TCP Protocol
============

TCP protocol starts with HTTP upgrade:

Handshake::

    GET / HTTP/1.1
    Connection: upgrade
    Upgrade: ciruela.v1

Response::

    HTTP/1.1 101 Switching Protocols


