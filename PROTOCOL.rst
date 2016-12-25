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


Handshake
---------

TCP protocol starts with HTTP upgrade:

Handshake::

    GET / HTTP/1.1
    Connection: upgrade
    Upgrade: ciruela.v1

Response::

    HTTP/1.1 101 Switching Protocols


Framing
-------

Framing of the protocol is simple: 8-byte big-endian message length
followed by payload of that size::

   +------------+-----------------+
   | size (64b) |     payload     |
   +------------+-----------------+


Serialization
-------------

Payload is serialized using CBOR_. Every packet contains at least two
consecutive values: packet type (string), and packet contents, usually a
dictionary, but it may be anything defined by packet type.

This serialization format is used for messages in both directions.


.. _cbor: http://cbor.io/
